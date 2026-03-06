//! The rendering engine for Vello integration.

use crate::core::{Color, Size};
use crate::graphics::Viewport;
use crate::layer::Layer;
use std::num::NonZeroUsize;
use std::sync::Arc;

#[cfg(any(feature = "image", feature = "svg"))]
use rustc_hash::FxHashMap;
#[cfg(any(feature = "image", feature = "svg"))]
use std::sync::RwLock;

use vello::kurbo::Affine;
use vello::wgpu;

pub use vello::Scene;

/// The rendering engine for Vello.
#[derive(Clone)]
pub struct Engine {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// The Vello renderer instance.
    renderer: Arc<std::sync::Mutex<vello::Renderer>>,
    /// Cached image data.
    #[cfg(feature = "image")]
    image_cache: Arc<RwLock<FxHashMap<crate::core::image::Id, ImageData>>>,
    /// Cached SVG data.
    #[cfg(feature = "svg")]
    svg_cache: Arc<RwLock<FxHashMap<u64, SvgData>>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Engine");

        #[cfg(feature = "image")]
        {
            let _ = debug.field("image_cache", &self.image_cache);
        }

        #[cfg(feature = "svg")]
        {
            let _ = debug.field("svg_cache", &self.svg_cache);
        }

        debug.finish()
    }
}

#[cfg(feature = "image")]
use vello::peniko::{Blob, ImageBrush, ImageData, ImageFormat};

#[derive(Debug, Clone)]
struct SvgData {
    width: u32,
    height: u32,
    // Store parsed SVG data here if needed
}

impl Engine {
    /// Creates a new [`Engine`].
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, _format: wgpu::TextureFormat) -> Self {
        // On macOS, use single-threaded initialization for better GPU compatibility.
        #[cfg(target_os = "macos")]
        const NUM_INIT_THREADS: Option<NonZeroUsize> = NonZeroUsize::new(1);
        #[cfg(not(target_os = "macos"))]
        const NUM_INIT_THREADS: Option<NonZeroUsize> = None;

        let renderer = vello::Renderer::new(
            &device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: NUM_INIT_THREADS,
                pipeline_cache: None,
            },
        )
        .expect("Failed to create Vello renderer");

        Self {
            device,
            queue,
            renderer: Arc::new(std::sync::Mutex::new(renderer)),
            #[cfg(feature = "image")]
            image_cache: Arc::new(RwLock::new(FxHashMap::default())),
            #[cfg(feature = "svg")]
            svg_cache: Arc::new(RwLock::new(FxHashMap::default())),
        }
    }

    /// Composes scenes from layers.
    pub fn compose_scenes(&self, layers: &[Layer], viewport: &Viewport) -> Scene {
        let mut final_scene = Scene::new();
        let scale = viewport.scale_factor() as f64;

        // Apply viewport scaling
        let viewport_transform = Affine::scale(scale);

        for layer in layers {
            // TODO: Apply layer clipping with push_layer/pop_layer
            final_scene.append(&layer.scene, Some(viewport_transform));
        }

        final_scene
    }

    /// Renders the scene to the target texture.
    pub fn render_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: &Scene,
        physical_size: Size<u32>,
        background_color: Color,
        _clear: bool,
    ) {
        // TODO: Vello currently creates its own CommandEncoder internally,
        // which means we can't use the provided encoder for batching commands.
        // This is a limitation of Vello's current API design.
        // Ideally, Vello would expose a lower-level API that accepts an encoder.

        // Create render parameters
        let render_params = vello::RenderParams {
            base_color: crate::layer::to_vello_color(background_color),
            width: physical_size.width,
            height: physical_size.height,
            // Use Area antialiasing for better performance
            // MSAA16 was causing 49x slowdown in present phase
            antialiasing_method: vello::AaConfig::Area,
        };

        // Render with Vello (this will create and submit its own commands)
        let mut renderer = self.renderer.lock().unwrap();
        renderer
            .render_to_texture(device, queue, scene, target, &render_params)
            .expect("Failed to render with Vello");
    }

    /// Loads an image and returns an ImageBrush for rendering.
    #[cfg(feature = "image")]
    pub fn load_image(&self, handle: &crate::core::image::Handle) -> Option<ImageBrush> {
        let id = handle.id();

        // Check if already cached
        {
            let cache = self.image_cache.read().unwrap();
            if let Some(image_data) = cache.get(&id) {
                return Some(image_data.clone().into());
            }
        }

        // Load the image using iced's graphics::image::load
        let image = crate::graphics::image::load(handle).ok()?;
        let width = image.width();
        let height = image.height();
        let rgba_bytes = image.into_raw();

        // Convert to Vello's ImageData format
        let blob = Blob::new(Arc::new(rgba_bytes));
        let image_data = ImageData {
            data: blob,
            format: ImageFormat::Rgba8,
            width,
            height,
            alpha_type: vello::peniko::ImageAlphaType::Alpha,
        };

        // Cache for future use
        {
            let mut cache = self.image_cache.write().unwrap();
            let _ = cache.insert(id.clone(), image_data.clone());
        }

        Some(image_data.into())
    }

    /// Measures the dimensions of an image.
    #[cfg(feature = "image")]
    pub fn measure_image(&self, handle: &crate::core::image::Handle) -> Option<Size<u32>> {
        let id = handle.id();

        // Check if already cached
        {
            let cache = self.image_cache.read().unwrap();
            if let Some(image_data) = cache.get(&id) {
                return Some(Size::new(image_data.width, image_data.height));
            }
        }

        // Load to get dimensions
        let image = crate::graphics::image::load(handle).ok()?;
        let width = image.width();
        let height = image.height();

        Some(Size::new(width, height))
    }

    /// Measures the dimensions of an SVG.
    #[cfg(feature = "svg")]
    pub fn measure_svg(&self, handle: &crate::core::svg::Handle) -> Size<u32> {
        // TODO: Implement proper SVG loading and caching
        Size::new(100, 100)
    }
}
