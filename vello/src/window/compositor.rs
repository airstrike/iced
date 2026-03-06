//! Window compositor implementation for Vello.

use crate::core::Color;
use crate::graphics::compositor::{self, Information, SurfaceError};
use crate::graphics::{self, Shell, Viewport, error};
use crate::settings::{self, Settings};
use crate::{Engine, Renderer};
use vello::wgpu;

/// A compositor error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// The surface creation failed.
    #[error("the surface creation failed: {0}")]
    SurfaceCreationFailed(#[from] wgpu::CreateSurfaceError),
    /// The surface is not compatible.
    #[error("the surface is not compatible")]
    IncompatibleSurface,
    /// No adapter was found for the options requested.
    #[error("no adapter was found for the options requested: {0:?}")]
    NoAdapterFound(String),
    /// No device request succeeded.
    #[error("no device request succeeded: {0:?}")]
    RequestDeviceFailed(Vec<(wgpu::Limits, wgpu::RequestDeviceError)>),
}

impl From<Error> for graphics::Error {
    fn from(error: Error) -> Self {
        Self::GraphicsAdapterNotFound {
            backend: "vello",
            reason: error::Reason::RequestFailed(error.to_string()),
        }
    }
}

/// Intermediate texture for a surface (required because compute shaders can't bind surface textures).
struct IntermediateTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// A window compositor using Vello for rendering.
pub struct Compositor {
    /// The wgpu instance.
    instance: wgpu::Instance,
    /// The wgpu adapter.
    adapter: wgpu::Adapter,
    /// The texture format used by surfaces.
    format: wgpu::TextureFormat,
    /// The alpha mode for surfaces.
    alpha_mode: wgpu::CompositeAlphaMode,
    /// The rendering engine.
    engine: Engine,
    /// The settings.
    settings: Settings,
    /// Intermediate textures for rendering (surface ID -> texture).
    /// Vello requires an intermediate texture because compute shaders can't bind surface textures.
    intermediate_textures: std::collections::HashMap<usize, IntermediateTexture>,
    /// Texture blitter for copying from intermediate to surface.
    blitter: wgpu::util::TextureBlitter,
}

impl Compositor {
    /// Requests a new [`Compositor`] with the given [`graphics::Settings`].
    ///
    /// Returns `None` if no compatible graphics adapter could be found.
    pub async fn request<W: compositor::Window>(
        settings: graphics::Settings,
        compatible_window: Option<W>,
        _shell: Shell,
    ) -> Result<Self, Error> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let compatible_surface =
            compatible_window.and_then(|window| instance.create_surface(window).ok());

        let adapter_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: compatible_surface.as_ref(),
        };

        let adapter = instance
            .request_adapter(&adapter_options)
            .await
            .map_err(|_error| Error::NoAdapterFound(format!("{adapter_options:?}")))?;

        let (format, alpha_mode) = compatible_surface
            .as_ref()
            .and_then(|surface| {
                let capabilities = surface.get_capabilities(&adapter);

                // Select format based on gamma correction setting, similar to iced_wgpu
                let mut formats = capabilities
                    .formats
                    .iter()
                    .copied()
                    .filter(|format| format.required_features() == wgpu::Features::empty());

                let format = if graphics::color::GAMMA_CORRECTION {
                    // Prefer sRGB formats when gamma correction is enabled
                    formats
                        .clone()
                        .find(wgpu::TextureFormat::is_srgb)
                        .or_else(|| formats.next())
                } else {
                    // Prefer non-sRGB formats when gamma correction is disabled
                    formats
                        .clone()
                        .find(|format| !wgpu::TextureFormat::is_srgb(format))
                        .or_else(|| formats.next())
                };

                let alpha_modes = capabilities.alpha_modes;
                let preferred_alpha =
                    if alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
                        wgpu::CompositeAlphaMode::PostMultiplied
                    } else if alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                        wgpu::CompositeAlphaMode::PreMultiplied
                    } else {
                        wgpu::CompositeAlphaMode::Auto
                    };

                format.zip(Some(preferred_alpha))
            })
            .ok_or(Error::IncompatibleSurface)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("iced_vello"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .map_err(|e| Error::RequestDeviceFailed(vec![(wgpu::Limits::default(), e)]))?;

        let engine = Engine::new(device, queue, format);

        // Create a texture blitter for copying from intermediate to surface
        let blitter = wgpu::util::TextureBlitter::new(&engine.device, format);

        Ok(Self {
            instance,
            adapter,
            format,
            alpha_mode,
            engine,
            settings: Settings::from(settings),
            intermediate_textures: std::collections::HashMap::new(),
            blitter,
        })
    }

    fn device(&self) -> &wgpu::Device {
        &self.engine.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.engine.queue
    }

    /// Creates a new [`Renderer`] for this [`Compositor`].
    pub fn create_renderer(&self) -> Renderer {
        Renderer::new(
            self.engine.clone(),
            self.settings.default_font,
            self.settings.default_text_size,
        )
    }

    /// Gets or creates an intermediate texture for the given surface and dimensions.
    /// Returns a cloned TextureView (cheap, just an Arc internally).
    fn get_or_create_intermediate_texture(
        &mut self,
        surface_id: usize,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        let device = self.device().clone();

        let entry = self
            .intermediate_textures
            .entry(surface_id)
            .or_insert_with(|| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("vello_intermediate_texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    // Use Rgba8Unorm which supports STORAGE_BINDING (Bgra8Unorm doesn't)
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });

                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

                IntermediateTexture { texture, view }
            });

        // Check if we need to recreate the texture due to size change
        if entry.texture.width() != width || entry.texture.height() != height {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vello_intermediate_texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Use Rgba8Unorm which supports STORAGE_BINDING (Bgra8Unorm doesn't)
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            *entry = IntermediateTexture { texture, view };
        }

        entry.view.clone()
    }
}

impl graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = wgpu::Surface<'static>;

    async fn with_backend(
        settings: graphics::Settings,
        _display: impl compositor::Display,
        compatible_window: impl compositor::Window,
        shell: Shell,
        backend: Option<&str>,
    ) -> Result<Self, graphics::Error> {
        match backend {
            None | Some("vello") => {
                let mut settings = Settings::from(settings);

                if let Some(present_mode) = settings::present_mode_from_env() {
                    settings.present_mode = present_mode;
                }

                Self::request(settings.into(), Some(compatible_window), shell)
                    .await
                    .map_err(graphics::Error::from)
            }
            Some(backend) => Err(graphics::Error::GraphicsAdapterNotFound {
                backend: "vello",
                reason: graphics::error::Reason::DidNotMatch {
                    preferred_backend: backend.to_owned(),
                },
            }),
        }
    }

    fn create_renderer(&self) -> Self::Renderer {
        self.create_renderer()
    }

    fn create_surface<W: compositor::Window>(
        &mut self,
        window: W,
        width: u32,
        height: u32,
    ) -> Self::Surface {
        let mut surface = self
            .instance
            .create_surface(window)
            .expect("Failed to create surface");

        if width > 0 && height > 0 {
            self.configure_surface(&mut surface, width, height);
        }

        surface
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        surface.configure(
            self.device(),
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                present_mode: self.settings.present_mode,
                width,
                height,
                alpha_mode: self.alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
            },
        );
    }

    fn information(&self) -> Information {
        let info = self.adapter.get_info();
        Information {
            adapter: info.name,
            backend: format!("{:?}", info.backend),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), SurfaceError> {
        let physical_size = viewport.physical_size();
        if physical_size.width == 0 || physical_size.height == 0 {
            return Ok(());
        }

        // Step 1: Render to intermediate texture (doesn't block)
        let render_timer = iced_debug::time("vello::present::render");
        let surface_id = surface as *const _ as usize;
        let intermediate_view = self.get_or_create_intermediate_texture(
            surface_id,
            physical_size.width,
            physical_size.height,
        );

        let mut dummy_encoder = self
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dummy_encoder_unused_by_vello"),
            });
        renderer.render(
            self.device(),
            self.queue(),
            &mut dummy_encoder,
            &intermediate_view,
            viewport,
            background_color,
            true,
        );
        render_timer.finish();

        // Step 2: Get surface texture (may block if frame latency limit reached)
        let surface_timer = iced_debug::time("vello::present::get_surface");
        let output = surface.get_current_texture().map_err(|e| match e {
            wgpu::SurfaceError::Timeout => SurfaceError::Timeout,
            wgpu::SurfaceError::Outdated => SurfaceError::Outdated,
            wgpu::SurfaceError::Lost => SurfaceError::Lost,
            wgpu::SurfaceError::OutOfMemory => SurfaceError::OutOfMemory,
            wgpu::SurfaceError::Other => SurfaceError::Other,
        })?;
        surface_timer.finish();

        // Step 3: Blit from intermediate texture to surface
        let blit_timer = iced_debug::time("vello::present::blit");
        let surface_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vello_blit_encoder"),
            });

        self.blitter.copy(
            self.device(),
            &mut encoder,
            &intermediate_view,
            &surface_view,
        );

        let _ = self.queue().submit([encoder.finish()]);
        blit_timer.finish();

        on_pre_present();

        // Step 4: Present
        let present_timer = iced_debug::time("vello::present::present_frame");
        output.present();
        present_timer.finish();

        Ok(())
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        renderer.screenshot(viewport, background_color)
    }

    fn load_font(&mut self, font: std::borrow::Cow<'static, [u8]>) {
        crate::text::load_font(font);
    }
}

use std::fmt;

impl fmt::Debug for Compositor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Compositor")
            .field("format", &self.format)
            .finish()
    }
}
