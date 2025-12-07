//! Global Parley context management for text rendering.

use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{FontContext, LayoutContext};
use std::borrow::Cow;
use std::sync::{Arc, OnceLock, RwLock};
use vello::peniko;

/// Global font context for Parley.
/// This manages font discovery and caching.
pub fn font_context() -> Arc<RwLock<FontContext>> {
    static FONT_CONTEXT: OnceLock<Arc<RwLock<FontContext>>> = OnceLock::new();
    FONT_CONTEXT
        .get_or_init(|| {
            // Create a collection with system fonts enabled
            let mut collection = Collection::new(CollectionOptions {
                shared: true,
                system_fonts: true,
            });

            // Load the Iced-Icons font that's used for checkmarks, arrows, etc.
            let iced_icons = include_bytes!("../../../graphics/fonts/Iced-Icons.ttf");
            let _ = collection.register_fonts(Blob::new(Arc::new(iced_icons.to_vec())), None);

            Arc::new(RwLock::new(FontContext {
                collection,
                source_cache: SourceCache::default(),
            }))
        })
        .clone()
}

/// Global layout context for Parley.
/// This provides scratch space for text layout operations.
pub fn layout_context() -> Arc<RwLock<LayoutContext<peniko::Brush>>> {
    static LAYOUT_CONTEXT: OnceLock<Arc<RwLock<LayoutContext<peniko::Brush>>>> = OnceLock::new();
    LAYOUT_CONTEXT
        .get_or_init(|| Arc::new(RwLock::new(LayoutContext::new())))
        .clone()
}

/// Loads a font into the global font context.
pub fn load_font(bytes: Cow<'static, [u8]>) {
    let font_ctx = font_context();
    let mut font_ctx = font_ctx.write().unwrap();

    // Register the font with Parley's fontique collection
    let _ = font_ctx
        .collection
        .register_fonts(Blob::new(Arc::new(bytes.into_owned())), None);
}
