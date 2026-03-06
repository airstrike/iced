//! The official renderer for iced.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "wgpu-bare")]
pub use iced_wgpu as wgpu;

#[cfg(feature = "vello")]
pub use iced_vello as vello;

pub mod fallback;

pub use iced_graphics as graphics;
pub use iced_graphics::core;

#[cfg(feature = "geometry")]
pub use iced_graphics::geometry;

/// The default graphics renderer for [`iced`].
///
/// [`iced`]: https://github.com/iced-rs/iced
pub type Renderer = renderer::Renderer;

/// The default graphics compositor for [`iced`].
///
/// [`iced`]: https://github.com/iced-rs/iced
pub type Compositor = renderer::Compositor;

#[cfg(all(feature = "wgpu-bare", feature = "tiny-skia", not(feature = "vello")))]
mod renderer {
    pub type Renderer = crate::fallback::Renderer<iced_wgpu::Renderer, iced_tiny_skia::Renderer>;

    pub type Compositor = crate::fallback::Compositor<
        iced_wgpu::window::Compositor,
        iced_tiny_skia::window::Compositor,
    >;
}

#[cfg(all(
    feature = "wgpu-bare",
    not(feature = "tiny-skia"),
    not(feature = "vello")
))]
mod renderer {
    pub type Renderer = iced_wgpu::Renderer;
    pub type Compositor = iced_wgpu::window::Compositor;
}

#[cfg(all(
    not(feature = "wgpu-bare"),
    feature = "tiny-skia",
    not(feature = "vello")
))]
mod renderer {
    pub type Renderer = iced_tiny_skia::Renderer;
    pub type Compositor = iced_tiny_skia::window::Compositor;
}

#[cfg(all(
    feature = "vello",
    not(feature = "wgpu-bare"),
    not(feature = "tiny-skia")
))]
mod renderer {
    pub type Renderer = iced_vello::Renderer;
    pub type Compositor = iced_vello::window::Compositor;
}

#[cfg(all(feature = "vello", feature = "wgpu-bare", not(feature = "tiny-skia")))]
mod renderer {
    // Use vello+vello (no actual fallback) to avoid text type incompatibility
    pub type Renderer = crate::fallback::Renderer<iced_vello::Renderer, iced_vello::Renderer>;

    pub type Compositor =
        crate::fallback::Compositor<iced_vello::window::Compositor, iced_vello::window::Compositor>;
}

#[cfg(all(feature = "vello", feature = "tiny-skia", not(feature = "wgpu-bare")))]
mod renderer {
    // Use vello+vello (no actual fallback) to avoid text type incompatibility
    pub type Renderer = crate::fallback::Renderer<iced_vello::Renderer, iced_vello::Renderer>;

    pub type Compositor =
        crate::fallback::Compositor<iced_vello::window::Compositor, iced_vello::window::Compositor>;
}

#[cfg(all(feature = "vello", feature = "wgpu-bare", feature = "tiny-skia"))]
mod renderer {
    // Use vello+vello (no actual fallback) to avoid text type incompatibility
    // Later, someone can implement a CPU-based Vello fallback
    pub type Renderer = crate::fallback::Renderer<iced_vello::Renderer, iced_vello::Renderer>;

    pub type Compositor =
        crate::fallback::Compositor<iced_vello::window::Compositor, iced_vello::window::Compositor>;
}

#[cfg(not(any(feature = "wgpu-bare", feature = "tiny-skia", feature = "vello")))]
mod renderer {
    #[cfg(not(debug_assertions))]
    compile_error!(
        "Cannot compile `iced_renderer` in release mode \
        without a renderer feature enabled. \
        Enable either the `wgpu`, `tiny-skia`, or `vello` feature."
    );

    pub type Renderer = ();
    pub type Compositor = ();
}
