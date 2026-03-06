//! Layer management for the Vello renderer.
//!
//! This module implements iced's layer system using Vello's Scene API.
//! Each layer builds its own Vello Scene, preserving draw order.

use crate::core::{
    self, Background, Color, Font, Pixels, Point, Rectangle, Transformation, renderer,
};
use crate::graphics;
use crate::graphics::color;
use crate::graphics::layer;
use crate::text::{Editor, Paragraph};
use crate::text_cache;

#[cfg(feature = "image")]
use std::cell::RefCell;
#[cfg(feature = "image")]
use std::sync::Arc;
use vello::Glyph;
use vello::kurbo::{self, Affine};
use vello::peniko;

/// A stack of layers for the Vello renderer.
pub type Stack = layer::Stack<Layer>;

/// A layer that builds a Vello Scene directly.
///
/// Instead of storing primitives to be batched later, we build
/// the Vello scene as we receive draw commands, preserving order.
#[derive(Clone)]
pub struct Layer {
    /// The bounds of the layer.
    pub bounds: Rectangle,
    /// The Vello scene being built for this layer.
    pub scene: vello::Scene,
    /// Pending text that needs cosmic-text processing.
    pub pending_text: Vec<Text>,
    /// Pending images that need to be loaded and drawn.
    #[cfg(feature = "image")]
    pub pending_images: Vec<Image>,
    /// Reference to the engine for loading images during flush.
    #[cfg(feature = "image")]
    pub(crate) engine: RefCell<Option<Arc<crate::Engine>>>,
    /// Current transformation stack for this layer.
    transforms: Vec<Transformation>,
    /// Track if we have quads (level 1)
    pub(crate) has_quads: bool,
    /// Track if we have text (level 5)
    pub(crate) has_text: bool,
    /// Text cache for simple text to avoid relayout.
    text_cache: text_cache::Cache,
}

/// A text primitive that needs to be processed.
///
/// Text is collected during the draw phase and processed
/// during flush to convert cosmic-text layouts to Vello glyphs.
#[derive(Debug, Clone)]
pub enum Text {
    /// Simple text.
    Simple {
        /// The text content.
        content: String,
        /// Position of the text.
        position: Point,
        /// Color of the text.
        color: Color,
        /// Size of the text.
        size: Pixels,
        /// Font of the text.
        font: Font,
        /// Horizontal alignment.
        align_x: core::text::Alignment,
        /// Vertical alignment.
        align_y: core::alignment::Vertical,
        /// Bounds for alignment.
        bounds: core::Size,
        /// Clip bounds.
        clip_bounds: Rectangle,
        /// Transformation.
        transformation: Transformation,
    },
    /// Paragraph text from our Vello text system.
    Paragraph {
        /// The paragraph.
        paragraph: crate::text::Weak,
        /// Position of the paragraph.
        position: Point,
        /// Color of the text.
        color: Color,
        /// Clip bounds.
        clip_bounds: Rectangle,
        /// Transformation.
        transformation: Transformation,
    },
    /// Editor text from our Vello text system.
    Editor {
        /// The editor.
        editor: crate::text::EditorWeak,
        /// Position of the editor.
        position: Point,
        /// Color of the text.
        color: Color,
        /// Clip bounds.
        clip_bounds: Rectangle,
        /// Transformation.
        transformation: Transformation,
    },
}

/// An image primitive that needs to be processed.
#[cfg(feature = "image")]
#[derive(Debug, Clone)]
pub struct Image {
    /// The image handle.
    pub handle: core::image::Handle,
    /// Bounds where the image should be drawn.
    pub bounds: Rectangle,
    /// Clip bounds.
    pub clip_bounds: Rectangle,
    /// Transformation.
    pub transformation: Transformation,
}

impl Layer {
    /// Gets the current combined transformation.
    fn current_transform(&self) -> Affine {
        // Combine all transformations in the stack
        self.transforms
            .iter()
            .fold(Affine::IDENTITY, |acc, t| acc * to_vello_transform(t))
    }

    /// Draws a quad directly to the scene.
    pub fn draw_quad(
        &mut self,
        quad: renderer::Quad,
        background: Background,
        transformation: Transformation,
    ) {
        self.has_quads = true;
        let transform = self.current_transform() * to_vello_transform(&transformation);
        let rect = kurbo::Rect::new(
            quad.bounds.x as f64,
            quad.bounds.y as f64,
            (quad.bounds.x + quad.bounds.width) as f64,
            (quad.bounds.y + quad.bounds.height) as f64,
        );

        // Check if we have border radius
        let has_radius = quad.border.radius.top_left > 0.0
            || quad.border.radius.top_right > 0.0
            || quad.border.radius.bottom_left > 0.0
            || quad.border.radius.bottom_right > 0.0;

        if has_radius {
            // Create rounded rectangle with radius
            let rounded_rect =
                kurbo::RoundedRect::from_rect(rect, quad.border.radius.top_left as f64);

            // Fill the background
            match background {
                Background::Color(color) => {
                    self.scene.fill(
                        peniko::Fill::NonZero,
                        transform,
                        to_vello_color(color),
                        None,
                        &rounded_rect,
                    );
                }
                Background::Gradient(_gradient) => {
                    // TODO: Implement gradient support
                    self.scene.fill(
                        peniko::Fill::NonZero,
                        transform,
                        peniko::Color::TRANSPARENT,
                        None,
                        &rounded_rect,
                    );
                }
            }

            // Draw border if needed
            if quad.border.width > 0.0 {
                let stroke = kurbo::Stroke::new(quad.border.width as f64);
                self.scene.stroke(
                    &stroke,
                    transform,
                    to_vello_color(quad.border.color),
                    None,
                    &rounded_rect,
                );
            }
        } else {
            // Use regular rectangle
            match background {
                Background::Color(color) => {
                    self.scene.fill(
                        peniko::Fill::NonZero,
                        transform,
                        to_vello_color(color),
                        None,
                        &rect,
                    );
                }
                Background::Gradient(_gradient) => {
                    // TODO: Implement gradient support
                    self.scene.fill(
                        peniko::Fill::NonZero,
                        transform,
                        peniko::Color::TRANSPARENT,
                        None,
                        &rect,
                    );
                }
            }

            // Draw border if needed
            if quad.border.width > 0.0 {
                let stroke = kurbo::Stroke::new(quad.border.width as f64);
                self.scene.stroke(
                    &stroke,
                    transform,
                    to_vello_color(quad.border.color),
                    None,
                    &rect,
                );
            }
        }
    }

    /// Draws a paragraph.
    pub fn draw_paragraph(
        &mut self,
        paragraph: &Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.has_text = true;
        self.pending_text.push(Text::Paragraph {
            paragraph: paragraph.downgrade(),
            position,
            color,
            clip_bounds,
            transformation,
        });
    }

    /// Draws an editor.
    pub fn draw_editor(
        &mut self,
        editor: &Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.has_text = true;
        self.pending_text.push(Text::Editor {
            editor: editor.downgrade(),
            position,
            color,
            clip_bounds,
            transformation,
        });
    }

    /// Draws text.
    pub fn draw_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.has_text = true;
        self.pending_text.push(Text::Simple {
            content: text.content,
            position,
            color,
            size: text.size,
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            bounds: text.bounds,
            clip_bounds,
            transformation,
        });
    }

    /// Draws an image.
    #[cfg(feature = "image")]
    pub fn draw_raster(
        &mut self,
        image: core::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.pending_images.push(Image {
            handle: image.handle,
            bounds,
            clip_bounds,
            transformation,
        });
    }

    /// Draws an SVG.
    #[cfg(feature = "svg")]
    pub fn draw_svg(
        &mut self,
        _svg: core::Svg,
        _bounds: Rectangle,
        _clip_bounds: Rectangle,
        _transformation: Transformation,
    ) {
        // TODO: Implement SVG rendering with Vello
        // Could use vello_svg for this
    }

    /// Draws geometry.
    #[cfg(feature = "geometry")]
    pub fn draw_geometry(
        &mut self,
        geometry: crate::geometry::Geometry,
        transformation: Transformation,
    ) {
        let transform = self.current_transform() * to_vello_transform(&transformation);

        match geometry {
            crate::geometry::Geometry::Live { scene } => {
                // Append the geometry scene to our layer scene
                self.scene.append(&scene, Some(transform));
            }
            crate::geometry::Geometry::Cached(cache) => {
                // Append the cached scene to our layer scene
                self.scene.append(&cache.scene, Some(transform));
            }
        }
    }

    /// Draws a mesh.
    #[cfg(feature = "geometry")]
    pub fn draw_mesh(&mut self, _mesh: graphics::Mesh, _transformation: Transformation) {
        // TODO: Convert mesh to Vello paths
        // This will require triangulation and path building
    }

    /// Processes pending text and adds it to the scene.
    fn flush_text(&mut self) {
        // Collect all text items to avoid borrow checker issues
        let text_items: Vec<_> = self.pending_text.drain(..).collect();

        // Process all pending text items and add them to the scene
        for text in text_items {
            match text {
                Text::Paragraph {
                    paragraph,
                    position,
                    color,
                    clip_bounds,
                    transformation,
                } => {
                    // Only render if we have a pre-built layout
                    if let Some(layout) = paragraph.layout() {
                        self.render_layout_clipped(
                            &layout,
                            position,
                            color,
                            clip_bounds,
                            transformation,
                        );
                    }
                }
                Text::Simple {
                    content,
                    position,
                    color,
                    size,
                    font,
                    align_x,
                    align_y,
                    bounds,
                    transformation,
                    ..
                } => {
                    // Use the cache to get or create a paragraph
                    let key = text_cache::Key {
                        content,
                        size,
                        line_height: core::text::LineHeight::default(),
                        font,
                        align_x,
                        align_y,
                        bounds,
                        wrapping: core::text::Wrapping::None, // No wrapping for simple text
                        shaping: core::text::Shaping::Basic,
                    };

                    let paragraph = self.text_cache.allocate(key);

                    if let Some(layout) = paragraph.layout() {
                        // Get the actual text size
                        use crate::core::text::Paragraph as _;
                        let text_size = paragraph.min_bounds();

                        // Adjust position based on alignment
                        // When align is Center, position refers to the center of the text
                        // When align is Left/Top, position refers to the top-left, etc.
                        let adjusted_x = match align_x {
                            core::text::Alignment::Left | core::text::Alignment::Default => {
                                position.x
                            }
                            core::text::Alignment::Center => position.x - text_size.width / 2.0,
                            core::text::Alignment::Right | core::text::Alignment::Justified => {
                                position.x - text_size.width
                            }
                        };

                        let adjusted_y = match align_y {
                            core::alignment::Vertical::Top => position.y,
                            core::alignment::Vertical::Center => {
                                position.y - text_size.height / 2.0
                            }
                            core::alignment::Vertical::Bottom => position.y - text_size.height,
                        };

                        let adjusted_position = Point::new(adjusted_x, adjusted_y);

                        self.render_layout(&layout, adjusted_position, color, transformation);
                    }
                }
                Text::Editor {
                    editor,
                    position,
                    color,
                    clip_bounds,
                    transformation,
                } => {
                    // Render the editor using its PlainEditor layout with clipping and scroll
                    if let Some(editor_strong) = editor.upgrade()
                        && let Some(layout) = editor_strong.editor().try_layout()
                    {
                        // Apply scroll offset to position
                        let scrolled_position =
                            Point::new(position.x, position.y - editor_strong.scroll_offset());

                        // Render with clipping bounds
                        self.render_layout_clipped(
                            layout,
                            scrolled_position,
                            color,
                            clip_bounds,
                            transformation,
                        );
                    }
                }
            }
        }
    }

    /// Processes pending images and adds them to the scene.
    #[cfg(feature = "image")]
    pub fn flush_images(&mut self, engine: &crate::Engine) {
        use vello::kurbo::Rect;

        // Collect all image items to avoid borrow checker issues
        let image_items: Vec<_> = self.pending_images.drain(..).collect();

        // Process all pending images and add them to the scene
        for image in image_items {
            // Load the image using the engine
            if let Some(image_brush) = engine.load_image(&image.handle) {
                // Calculate the base transform
                let base_transform =
                    self.current_transform() * to_vello_transform(&image.transformation);

                // Apply clipping with push_layer
                let clip_rect = Rect::new(
                    image.clip_bounds.x as f64,
                    image.clip_bounds.y as f64,
                    (image.clip_bounds.x + image.clip_bounds.width) as f64,
                    (image.clip_bounds.y + image.clip_bounds.height) as f64,
                );

                self.scene.push_clip_layer(base_transform, &clip_rect);

                // Now apply the image transform (translation + scaling)
                let transform = base_transform
                    * Affine::translate((image.bounds.x as f64, image.bounds.y as f64));

                // Apply scaling to fit the image into the bounds
                // The image data has intrinsic size, so we need to scale it to match bounds
                if let Some(size) = engine.measure_image(&image.handle) {
                    let scale_x = image.bounds.width as f64 / size.width as f64;
                    let scale_y = image.bounds.height as f64 / size.height as f64;
                    let transform = transform * Affine::scale_non_uniform(scale_x, scale_y);

                    // Draw the image to the scene
                    self.scene.draw_image(&image_brush, transform);
                } else {
                    // If we can't get the size, just draw without scaling
                    self.scene.draw_image(&image_brush, transform);
                }

                // Pop the clipping layer
                self.scene.pop_layer();
            }
        }
    }

    /// Render a Parley layout to the scene with clipping
    fn render_layout_clipped(
        &mut self,
        layout: &parley::Layout<vello::peniko::Brush>,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        use parley::layout::PositionedLayoutItem;
        use vello::kurbo::Rect;

        let transform = self.current_transform() * to_vello_transform(&transformation);

        // Apply clipping with push_layer
        let clip_rect = Rect::new(
            clip_bounds.x as f64,
            clip_bounds.y as f64,
            (clip_bounds.x + clip_bounds.width) as f64,
            (clip_bounds.y + clip_bounds.height) as f64,
        );

        self.scene.push_clip_layer(transform, &clip_rect);

        let transform = transform * Affine::translate((position.x as f64, position.y as f64));

        // Calculate visible Y range in layout space for culling
        let clip_y_min = clip_bounds.y as f64;
        let clip_y_max = (clip_bounds.y + clip_bounds.height) as f64;

        // Iterate through lines and glyph runs, skipping lines outside visible area
        for line in layout.lines() {
            let metrics = line.metrics();

            // Calculate line's Y bounds in screen space
            // baseline is the reference point, ascent goes up (negative), descent goes down (positive)
            let line_y_top = (metrics.baseline - metrics.ascent) as f64;
            let line_y_bottom = (metrics.baseline + metrics.descent) as f64;

            // Apply position offset to get screen coordinates
            let screen_y_top = position.y as f64 + line_y_top;
            let screen_y_bottom = position.y as f64 + line_y_bottom;

            // Skip if line is completely outside visible area
            if screen_y_bottom < clip_y_min || screen_y_top > clip_y_max {
                continue;
            }

            // Line is visible, render its glyphs
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let synthesis = run.synthesis();

                // Handle oblique (fake italic) if needed
                let glyph_transform = synthesis
                    .skew()
                    .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));

                // Draw the glyphs
                self.scene
                    .draw_glyphs(font)
                    .font_size(font_size)
                    .transform(transform)
                    .glyph_transform(glyph_transform)
                    .normalized_coords(run.normalized_coords())
                    .brush(to_vello_color(color))
                    .draw(
                        peniko::Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y - glyph.y;
                            x += glyph.advance;
                            Glyph {
                                id: glyph.id,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }

        // Pop the clipping layer
        self.scene.pop_layer();
    }

    /// Render a Parley layout to the scene
    fn render_layout(
        &mut self,
        layout: &parley::Layout<vello::peniko::Brush>,
        position: Point,
        color: Color,
        transformation: Transformation,
    ) {
        use parley::layout::PositionedLayoutItem;

        let transform = self.current_transform() * to_vello_transform(&transformation);
        let transform = transform * Affine::translate((position.x as f64, position.y as f64));

        // Iterate through lines and glyph runs
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let synthesis = run.synthesis();

                // Handle oblique (fake italic) if needed
                let glyph_transform = synthesis
                    .skew()
                    .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));

                // Draw the glyphs
                self.scene
                    .draw_glyphs(font)
                    .font_size(font_size)
                    .transform(transform)
                    .glyph_transform(glyph_transform)
                    .normalized_coords(run.normalized_coords())
                    .brush(to_vello_color(color))
                    .draw(
                        peniko::Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y - glyph.y;
                            x += glyph.advance;
                            Glyph {
                                id: glyph.id,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            bounds: Rectangle::INFINITE,
            scene: vello::Scene::new(),
            pending_text: Vec::new(),
            #[cfg(feature = "image")]
            pending_images: Vec::new(),
            #[cfg(feature = "image")]
            engine: RefCell::new(None),
            transforms: vec![Transformation::IDENTITY],
            has_quads: false,
            has_text: false,
            text_cache: text_cache::Cache::new(),
        }
    }
}

impl std::fmt::Debug for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layer")
            .field("bounds", &self.bounds)
            .field("pending_text", &self.pending_text)
            .field("transforms", &self.transforms)
            .finish()
    }
}

impl graphics::Layer for Layer {
    fn with_bounds(bounds: Rectangle) -> Self {
        Self {
            bounds,
            ..Default::default()
        }
    }

    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn flush(&mut self) {
        #[cfg(feature = "image")]
        {
            // Clone the engine Arc to avoid borrow checker issues
            let engine_opt = self.engine.borrow().clone();
            if let Some(engine) = engine_opt {
                self.flush_images(&engine);
            }
        }

        self.flush_text();
    }

    fn resize(&mut self, bounds: Rectangle) {
        self.bounds = bounds;
    }

    fn reset(&mut self) {
        self.bounds = Rectangle::INFINITE;
        self.scene = vello::Scene::new();
        self.pending_text.clear();
        #[cfg(feature = "image")]
        self.pending_images.clear();
        self.transforms = vec![Transformation::IDENTITY];
        self.has_quads = false;
        self.has_text = false;
    }

    fn start(&self) -> usize {
        // Return the lowest level that has content
        if self.has_quads {
            return 1;
        }

        if self.has_text || !self.pending_text.is_empty() {
            return 5;
        }

        usize::MAX
    }

    fn end(&self) -> usize {
        // Return the highest level that has content
        if self.has_text || !self.pending_text.is_empty() {
            return 5;
        }

        if self.has_quads {
            return 1;
        }

        0
    }

    fn merge(&mut self, other: &mut Self) {
        // Append the other layer's scene to this one
        self.scene.append(&other.scene, None);
        self.pending_text.append(&mut other.pending_text);
        self.has_quads = self.has_quads || other.has_quads;
        self.has_text = self.has_text || other.has_text;
    }
}

/// Helper functions for converting between iced and Vello types
pub(crate) fn to_vello_transform(transformation: &Transformation) -> Affine {
    // Transformation in iced is already a 4x4 matrix
    // We need to convert it to Vello's Affine (2D transform)
    let scale = transformation.scale_factor();
    let translation = transformation.translation();

    Affine::translate((translation.x as f64, translation.y as f64)) * Affine::scale(scale as f64)
}

pub(crate) fn to_vello_color(color: Color) -> peniko::Color {
    let [r, g, b, a] = color::pack(color).components();
    peniko::Color::from_rgba8(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    )
}
