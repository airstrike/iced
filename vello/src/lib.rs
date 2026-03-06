//! A GPU-accelerated renderer for iced using Vello.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/iced-rs/iced/9ab6923e943f784985e9ef9ca28b10278297225d/docs/logo.svg"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod engine;
#[cfg(feature = "geometry")]
pub mod geometry;
pub mod layer;
pub mod settings;
pub mod text;
pub mod text_cache;
pub mod window;

pub use engine::Engine;
pub use layer::Layer;
pub use settings::Settings;

pub use iced_graphics as graphics;
pub use iced_graphics::core;

use crate::core::{Background, Color, Font, Pixels, Point, Rectangle, Transformation};
use crate::graphics::Viewport;
use crate::text::{Editor, Paragraph};

use iced_debug as debug;
use vello::wgpu;

/// A Vello-based GPU-accelerated renderer.
#[derive(Debug)]
pub struct Renderer {
    /// The underlying engine for rendering.
    engine: Engine,
    /// Stack of layers for rendering.
    layers: layer::Stack,
    /// Default font.
    default_font: Font,
    /// Default text size.
    default_text_size: Pixels,
}

impl Renderer {
    /// Creates a new [`Renderer`].
    pub fn new(engine: Engine, default_font: Font, default_text_size: Pixels) -> Self {
        Self {
            engine,
            layers: layer::Stack::new(),
            default_font,
            default_text_size,
        }
    }

    /// Returns a reference to the [`Engine`].
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Returns a mutable reference to the [`Engine`].
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// Renders the current state to the target texture.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: &Viewport,
        background_color: Color,
        clear: bool,
    ) {
        let physical_size = viewport.physical_size();

        if physical_size.width == 0 || physical_size.height == 0 {
            return;
        }

        // Set the engine reference in all layers so they can flush images
        #[cfg(feature = "image")]
        {
            let engine_ref = std::sync::Arc::new(self.engine.clone());
            for layer in self.layers.iter() {
                *layer.engine.borrow_mut() = Some(engine_ref.clone());
            }
        }

        // Flush any pending text and images in all layers
        let flush_timer = debug::time("vello::flush");
        self.layers.flush();
        flush_timer.finish();

        // Compose all layer scenes into a single scene
        let compose_timer = debug::time("vello::compose");
        let scene = self.engine.compose_scenes(self.layers.as_slice(), viewport);
        compose_timer.finish();

        // Render the composed scene using Vello
        let render_timer = debug::time("vello::render");
        self.engine.render_scene(
            device,
            queue,
            encoder,
            target,
            &scene,
            physical_size,
            background_color,
            clear,
        );
        render_timer.finish();

        // Reset all layer scenes to free memory allocations, preventing
        // scenes from growing across frames.
        for layer in self.layers.as_slice_mut() {
            layer.scene.reset();
        }
    }

    /// Takes a screenshot of the current renderer state.
    pub fn screenshot(&mut self, _viewport: &Viewport, _background_color: Color) -> Vec<u8> {
        // TODO: Implement screenshot functionality
        vec![]
    }
}

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push_clip(bounds);
    }

    fn end_layer(&mut self) {
        self.layers.pop_clip();
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        self.layers.push_transformation(transformation);
    }

    fn end_transformation(&mut self) {
        self.layers.pop_transformation();
    }

    fn fill_quad(&mut self, quad: core::renderer::Quad, background: impl Into<Background>) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into(), transformation);
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        self.layers.reset(new_bounds);
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        _callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        #[cfg(feature = "image")]
        {
            // Load the image to get its size, then create an allocation
            match crate::graphics::image::load(_handle) {
                Ok(image) => {
                    let width = image.width();
                    let height = image.height();

                    #[allow(unsafe_code)]
                    let allocation =
                        unsafe { core::image::allocate(_handle, core::Size::new(width, height)) };

                    _callback(Ok(allocation));
                }
                Err(error) => {
                    _callback(Err(error));
                }
            }
        }
    }
}

impl core::text::Renderer for Renderer {
    type Font = Font;
    type Paragraph = Paragraph;
    type Editor = Editor;

    const ICON_FONT: Font = Font::with_name("Iced-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const SCROLL_UP_ICON: char = '\u{e802}';
    const SCROLL_DOWN_ICON: char = '\u{e803}';
    const SCROLL_LEFT_ICON: char = '\u{e804}';
    const SCROLL_RIGHT_ICON: char = '\u{e805}';
    const ICED_LOGO: char = '\u{e801}';

    fn default_font(&self) -> Self::Font {
        self.default_font
    }

    fn default_size(&self) -> Pixels {
        self.default_text_size
    }

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_paragraph(text, position, color, clip_bounds, transformation);
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        _handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        // TODO: Implement image loading
        Err(core::image::Error::Invalid(std::sync::Arc::new(
            std::io::Error::new(std::io::ErrorKind::Other, "Not implemented"),
        )))
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<core::Size<u32>> {
        self.engine.measure_image(handle)
    }

    fn draw_image(&mut self, image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> core::Size<u32> {
        self.engine.measure_svg(handle)
    }

    fn draw_svg(&mut self, svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = geometry::Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_geometry(geometry, transformation);
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, mesh: graphics::Mesh) {
        let (_layer, transformation) = self.layers.current_mut();
        // TODO: Implement mesh rendering
        let _ = (mesh, transformation);
    }

    fn draw_mesh_cache(&mut self, _cache: graphics::mesh::Cache) {
        // TODO: Implement mesh cache rendering
    }
}

impl graphics::compositor::Default for Renderer {
    type Compositor = window::Compositor;
}

impl core::renderer::Headless for Renderer {
    async fn new(
        default_font: Font,
        default_text_size: Pixels,
        backend: Option<&str>,
    ) -> Option<Self> {
        if backend.is_some_and(|backend| backend != "vello") {
            return None;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("iced_vello [headless]"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .ok()?;

        let format = if graphics::color::GAMMA_CORRECTION {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        let engine = Engine::new(device, queue, format);

        Some(Self::new(engine, default_font, default_text_size))
    }

    fn name(&self) -> String {
        "vello".to_owned()
    }

    fn screenshot(
        &mut self,
        size: core::Size<u32>,
        _scale_factor: f32,
        _background_color: Color,
    ) -> Vec<u8> {
        // TODO: Implement screenshot
        let pixel_count = (size.width * size.height * 4) as usize;
        vec![0; pixel_count]
    }
}
