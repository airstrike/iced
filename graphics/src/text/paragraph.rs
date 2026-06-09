//! Draw paragraphs.
use crate::core;
use crate::core::alignment;
use crate::core::font;
use crate::core::text::{
    Alignment, Decoration, Ellipsis, Hit, LineHeight, Shaping, Span, Text, Wrapping,
};
use crate::core::{Em, Font, Pixels, Point, Rectangle, Size};
use crate::text;

use std::fmt;
use std::sync::{self, Arc};

/// A bunch of text.
#[derive(Clone, PartialEq)]
pub struct Paragraph(Arc<Internal>);

#[derive(Clone)]
struct Internal {
    buffer: cosmic_text::Buffer,
    font: Font,
    shaping: Shaping,
    wrapping: Wrapping,
    ellipsis: Ellipsis,
    align_x: Alignment,
    align_y: alignment::Vertical,
    bounds: Size,
    min_bounds: Size,
    version: text::Version,
    letter_spacing: Em,
    font_features: Vec<font::Feature>,
    font_variations: Vec<font::Variation>,
    hint: bool,
    hint_factor: f32,
}

impl Paragraph {
    /// Creates a new empty [`Paragraph`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the buffer of the [`Paragraph`].
    pub fn buffer(&self) -> &cosmic_text::Buffer {
        &self.internal().buffer
    }

    /// Creates a [`Weak`] reference to the [`Paragraph`].
    ///
    /// This is useful to avoid cloning the [`Paragraph`] when
    /// referential guarantees are unnecessary. For instance,
    /// when creating a rendering tree.
    pub fn downgrade(&self) -> Weak {
        let paragraph = self.internal();

        Weak {
            raw: Arc::downgrade(paragraph),
            min_bounds: paragraph.min_bounds,
            align_x: paragraph.align_x,
            align_y: paragraph.align_y,
        }
    }

    fn internal(&self) -> &Arc<Internal> {
        &self.0
    }
}

impl core::text::Paragraph for Paragraph {
    type Font = Font;

    fn with_text(text: Text<&str>) -> Self {
        log::trace!("Allocating plain paragraph: {}", text.content);

        let mut font_system = text::font_system().write().expect("Write font system");

        let (hint, hint_factor) = match text::hint_factor(text.size, text.hint_factor) {
            Some(hint_factor) => (true, hint_factor),
            _ => (false, 1.0),
        };

        let mut buffer = cosmic_text::Buffer::new(
            font_system.raw(),
            cosmic_text::Metrics::new(
                f32::from(text.size) * hint_factor,
                f32::from(text.line_height.to_absolute(text.size)) * hint_factor,
            ),
        );

        if hint {
            buffer.set_hinting(cosmic_text::Hinting::Enabled);
        }

        buffer.set_size(
            Some(text.bounds.width * hint_factor),
            Some(text.bounds.height * hint_factor),
        );

        buffer.set_wrap(text::to_wrap(text.wrapping));
        buffer.set_ellipsize(text::to_ellipsize(
            text.ellipsis,
            text.bounds.height * hint_factor,
        ));

        let font = match text.weight {
            Some(weight) => text.font.weight(weight),
            None => text.font,
        };

        buffer.set_text(
            text.content,
            &text::to_attributes(
                font,
                text.letter_spacing,
                &text.font_features,
                &text.font_variations,
            ),
            text::to_shaping(
                text.shaping,
                text.content,
                !text.font_features.is_empty() || !text.font_variations.is_empty(),
            ),
            None,
        );
        buffer.shape_until_scroll(font_system.raw(), false);

        let min_bounds = text::align(&mut buffer, font_system.raw(), text.align_x) / hint_factor;

        Self(Arc::new(Internal {
            buffer,
            hint,
            hint_factor,
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            shaping: text.shaping,
            wrapping: text.wrapping,
            ellipsis: text.ellipsis,
            bounds: text.bounds,
            min_bounds,
            version: font_system.version(),
            letter_spacing: text.letter_spacing,
            font_features: text.font_features,
            font_variations: text.font_variations,
        }))
    }

    fn with_spans<Link>(text: Text<&[Span<'_, Link>]>) -> Self {
        log::trace!("Allocating rich paragraph: {} spans", text.content.len());

        let mut font_system = text::font_system().write().expect("Write font system");

        let (hint, hint_factor) = match text::hint_factor(text.size, text.hint_factor) {
            Some(hint_factor) => (true, hint_factor),
            _ => (false, 1.0),
        };

        let mut buffer = cosmic_text::Buffer::new(
            font_system.raw(),
            cosmic_text::Metrics::new(
                f32::from(text.size) * hint_factor,
                f32::from(text.line_height.to_absolute(text.size)) * hint_factor,
            ),
        );

        if hint {
            buffer.set_hinting(cosmic_text::Hinting::Enabled);
        }

        buffer.set_size(
            Some(text.bounds.width * hint_factor),
            Some(text.bounds.height * hint_factor),
        );

        buffer.set_wrap(text::to_wrap(text.wrapping));

        buffer.set_rich_text(
            text.content.iter().enumerate().map(|(i, span)| {
                let span_features = if span.font_features.is_empty() {
                    &text.font_features
                } else {
                    &span.font_features
                };

                let span_variations = if span.font_variations.is_empty() {
                    &text.font_variations
                } else {
                    &span.font_variations
                };

                let attrs = text::to_attributes(
                    span.font.unwrap_or(text.font),
                    span.letter_spacing.unwrap_or(text.letter_spacing),
                    span_features,
                    span_variations,
                );

                let attrs = match (span.size, span.line_height) {
                    (None, None) => attrs,
                    _ => {
                        let size = span.size.unwrap_or(text.size);

                        attrs.metrics(cosmic_text::Metrics::new(
                            f32::from(size) * hint_factor,
                            f32::from(
                                span.line_height
                                    .unwrap_or(text.line_height)
                                    .to_absolute(size),
                            ) * hint_factor,
                        ))
                    }
                };

                let attrs = if let Some(color) = span.color {
                    attrs.color(text::to_color(color))
                } else {
                    attrs
                };

                (span.text.as_ref(), attrs.metadata(i))
            }),
            &text::to_attributes(
                text.font,
                text.letter_spacing,
                &text.font_features,
                &text.font_variations,
            ),
            cosmic_text::Shaping::Advanced,
            None,
        );

        buffer.shape_until_scroll(font_system.raw(), false);

        let min_bounds = text::align(&mut buffer, font_system.raw(), text.align_x) / hint_factor;

        Self(Arc::new(Internal {
            buffer,
            hint,
            hint_factor,
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            shaping: text.shaping,
            wrapping: text.wrapping,
            ellipsis: text.ellipsis,
            bounds: text.bounds,
            min_bounds,
            version: font_system.version(),
            letter_spacing: text.letter_spacing,
            font_features: text.font_features,
            font_variations: text.font_variations,
        }))
    }

    fn resize(&mut self, new_bounds: Size) {
        let paragraph = Arc::make_mut(&mut self.0);

        let mut font_system = text::font_system().write().expect("Write font system");

        paragraph.buffer.set_size(
            Some(new_bounds.width * paragraph.hint_factor),
            Some(new_bounds.height * paragraph.hint_factor),
        );
        paragraph
            .buffer
            .shape_until_scroll(font_system.raw(), false);

        let min_bounds = text::align(&mut paragraph.buffer, font_system.raw(), paragraph.align_x)
            / paragraph.hint_factor;

        paragraph.bounds = new_bounds;
        paragraph.min_bounds = min_bounds;
    }

    fn compare(&self, text: Text<()>) -> core::text::Difference {
        let font_system = text::font_system().read().expect("Read font system");
        let paragraph = self.internal();
        let metrics = paragraph.buffer.metrics();

        if paragraph.version != font_system.version
            || metrics.font_size != text.size.0 * paragraph.hint_factor
            || metrics.line_height
                != text.line_height.to_absolute(text.size).0 * paragraph.hint_factor
            || paragraph.font != text.font
            || paragraph.shaping != text.shaping
            || paragraph.wrapping != text.wrapping
            || paragraph.ellipsis != text.ellipsis
            || paragraph.letter_spacing != text.letter_spacing
            || paragraph.font_features != text.font_features
            || paragraph.font_variations != text.font_variations
            || paragraph.align_x != text.align_x
            || paragraph.align_y != text.align_y
            || paragraph.hint.then_some(paragraph.hint_factor)
                != text::hint_factor(text.size, text.hint_factor)
        {
            core::text::Difference::Shape
        } else if paragraph.bounds != text.bounds {
            core::text::Difference::Bounds
        } else {
            core::text::Difference::None
        }
    }

    fn hint_factor(&self) -> Option<f32> {
        self.0.hint.then_some(self.0.hint_factor)
    }

    fn size(&self) -> Pixels {
        Pixels(self.0.buffer.metrics().font_size / self.0.hint_factor)
    }

    fn font(&self) -> Font {
        self.0.font
    }

    fn line_height(&self) -> LineHeight {
        LineHeight::Absolute(Pixels(
            self.0.buffer.metrics().line_height / self.0.hint_factor,
        ))
    }

    fn align_x(&self) -> Alignment {
        self.internal().align_x
    }

    fn align_y(&self) -> alignment::Vertical {
        self.internal().align_y
    }

    fn wrapping(&self) -> Wrapping {
        self.0.wrapping
    }

    fn ellipsis(&self) -> Ellipsis {
        self.0.ellipsis
    }

    fn shaping(&self) -> Shaping {
        self.0.shaping
    }

    fn letter_spacing(&self) -> Em {
        self.0.letter_spacing
    }

    fn font_features(&self) -> &[font::Feature] {
        &self.0.font_features
    }

    fn font_variations(&self) -> &[font::Variation] {
        &self.0.font_variations
    }

    fn bounds(&self) -> Size {
        self.0.bounds
    }

    fn min_bounds(&self) -> Size {
        self.internal().min_bounds
    }

    fn hit_test(&self, point: Point) -> Option<Hit> {
        let cursor = self
            .internal()
            .buffer
            .hit(point.x * self.0.hint_factor, point.y * self.0.hint_factor)?;

        Some(Hit::CharOffset(cursor.index))
    }

    fn hit_span(&self, point: Point) -> Option<usize> {
        let internal = self.internal();

        let cursor = internal
            .buffer
            .hit(point.x * self.0.hint_factor, point.y * self.0.hint_factor)?;
        let line = internal.buffer.lines.get(cursor.line)?;

        if cursor.index >= line.text().len() {
            return None;
        }

        let index = match cursor.affinity {
            cosmic_text::Affinity::Before => cursor.index.saturating_sub(1),
            cosmic_text::Affinity::After => cursor.index,
        };

        let mut hit = None;
        let glyphs = line
            .layout_opt()
            .as_ref()?
            .iter()
            .flat_map(|line| line.glyphs.iter());

        for glyph in glyphs {
            if glyph.start <= index && index < glyph.end {
                hit = Some(glyph);
                break;
            }
        }

        Some(hit?.metadata)
    }

    fn span_bounds(&self, index: usize) -> Vec<Rectangle> {
        let internal = self.internal();

        let mut bounds = Vec::new();
        let mut current_bounds = None;

        let glyphs = internal
            .buffer
            .layout_runs()
            .flat_map(|run| {
                let line_top = run.line_top;
                let line_height = run.line_height;

                run.glyphs
                    .iter()
                    .map(move |glyph| (line_top, line_height, glyph))
            })
            .skip_while(|(_, _, glyph)| glyph.metadata != index)
            .take_while(|(_, _, glyph)| glyph.metadata == index);

        for (line_top, line_height, glyph) in glyphs {
            let y = line_top + glyph.y;

            let new_bounds = || {
                Rectangle::new(
                    Point::new(glyph.x, y),
                    Size::new(glyph.w, glyph.line_height_opt.unwrap_or(line_height)),
                ) * (1.0 / self.0.hint_factor)
            };

            match current_bounds.as_mut() {
                None => {
                    current_bounds = Some(new_bounds());
                }
                Some(current_bounds) if y != current_bounds.y => {
                    bounds.push(*current_bounds);
                    *current_bounds = new_bounds();
                }
                Some(current_bounds) => {
                    current_bounds.width += glyph.w / self.0.hint_factor;
                }
            }
        }

        bounds.extend(current_bounds);
        bounds
    }

    fn grapheme_position(&self, line: usize, index: usize) -> Option<Point> {
        use unicode_segmentation::UnicodeSegmentation;

        let run = self.internal().buffer.layout_runs().nth(line)?;

        // index represents a grapheme, not a glyph
        // Let's find the first glyph for the given grapheme cluster
        let mut last_start = None;
        let mut last_grapheme_count = 0;
        let mut graphemes_seen = 0;

        let glyph = run
            .glyphs
            .iter()
            .find(|glyph| {
                if Some(glyph.start) != last_start {
                    last_grapheme_count = run.text[glyph.start..glyph.end].graphemes(false).count();
                    last_start = Some(glyph.start);
                    graphemes_seen += last_grapheme_count;
                }

                graphemes_seen >= index
            })
            .or_else(|| run.glyphs.last())?;

        let advance = if index == 0 {
            0.0
        } else {
            glyph.w
                * (1.0
                    - graphemes_seen.saturating_sub(index) as f32
                        / last_grapheme_count.max(1) as f32)
        };

        Some(Point::new(
            (glyph.x + glyph.x_offset * glyph.font_size + advance) / self.0.hint_factor,
            (glyph.y - glyph.y_offset * glyph.font_size) / self.0.hint_factor,
        ))
    }

    fn recolor_span(&mut self, index: usize, color: Option<core::Color>) {
        // Spans are tagged with their index via `metadata(i)` in `with_spans`,
        // so cosmic-text recolors the right glyphs in place without reshaping.
        let internal = Arc::make_mut(&mut self.0);
        let _ = internal
            .buffer
            .recolor_metadata(index, color.map(text::to_color));
    }

    fn decoration_bounds(&self, index: usize, decoration: Decoration) -> Vec<Rectangle> {
        let internal = self.internal();
        let hint_factor = self.0.hint_factor;

        let Ok(mut font_system) = text::font_system().write() else {
            return Vec::new();
        };
        let font_system = font_system.raw();

        let mut bounds = Vec::new();

        for run in internal.buffer.layout_runs() {
            let mut x_min = f32::MAX;
            let mut x_max = f32::MIN;
            let mut font = None;

            for glyph in run.glyphs.iter().filter(|glyph| glyph.metadata == index) {
                x_min = x_min.min(glyph.x);
                x_max = x_max.max(glyph.x + glyph.w);
                font = font.or(Some((glyph.font_id, glyph.font_size)));
            }

            let Some((font_id, font_size)) = font else {
                continue;
            };
            let Some(metrics) = font_system.decoration_metrics(font_id) else {
                continue;
            };

            // Mirrors cosmic-text's `render_decoration`: y from the baseline
            // (`line_y`) minus the font's offset, thickness from the font.
            let line = |y: f32, thickness: f32| {
                Rectangle::new(
                    Point::new(x_min, y),
                    Size::new(x_max - x_min, thickness.max(1.0)),
                ) * (1.0 / hint_factor)
            };
            let underline = metrics.underline.thickness * font_size;

            match decoration {
                Decoration::Underline => {
                    bounds.push(line(
                        run.line_y - metrics.underline.offset * font_size,
                        underline,
                    ));
                }
                Decoration::DoubleUnderline => {
                    let y = run.line_y - metrics.underline.offset * font_size;
                    bounds.push(line(y, underline));
                    bounds.push(line(y + underline * 2.0, underline));
                }
                Decoration::Strikethrough => {
                    let y = run.line_y - metrics.strikethrough.offset * font_size;
                    bounds.push(line(y, metrics.strikethrough.thickness * font_size));
                }
                Decoration::Overline => {
                    let y = (run.line_y - metrics.ascent * font_size).max(run.line_top);
                    bounds.push(line(y, underline));
                }
            }
        }

        bounds
    }
}

impl Default for Paragraph {
    fn default() -> Self {
        Self(Arc::new(Internal::default()))
    }
}

impl fmt::Debug for Paragraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paragraph = self.internal();

        f.debug_struct("Paragraph")
            .field("font", &paragraph.font)
            .field("shaping", &paragraph.shaping)
            .field("horizontal_alignment", &paragraph.align_x)
            .field("vertical_alignment", &paragraph.align_y)
            .field("bounds", &paragraph.bounds)
            .field("min_bounds", &paragraph.min_bounds)
            .finish()
    }
}

impl PartialEq for Internal {
    fn eq(&self, other: &Self) -> bool {
        self.font == other.font
            && self.shaping == other.shaping
            && self.align_x == other.align_x
            && self.align_y == other.align_y
            && self.bounds == other.bounds
            && self.min_bounds == other.min_bounds
            && self.buffer.metrics() == other.buffer.metrics()
    }
}

impl Default for Internal {
    fn default() -> Self {
        Self {
            buffer: cosmic_text::Buffer::new_empty(cosmic_text::Metrics {
                font_size: 1.0,
                line_height: 1.0,
            }),
            font: Font::default(),
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
            ellipsis: Ellipsis::default(),
            align_x: Alignment::Default,
            align_y: alignment::Vertical::Top,
            bounds: Size::ZERO,
            min_bounds: Size::ZERO,
            version: text::Version::default(),
            letter_spacing: Em::ZERO,
            font_features: Vec::new(),
            font_variations: Vec::new(),
            hint: false,
            hint_factor: 1.0,
        }
    }
}

/// A weak reference to a [`Paragraph`].
#[derive(Debug, Clone)]
pub struct Weak {
    raw: sync::Weak<Internal>,
    /// The minimum bounds of the [`Paragraph`].
    pub min_bounds: Size,
    /// The horizontal alignment of the [`Paragraph`].
    pub align_x: Alignment,
    /// The vertical alignment of the [`Paragraph`].
    pub align_y: alignment::Vertical,
}

impl Weak {
    /// Tries to update the reference into a [`Paragraph`].
    pub fn upgrade(&self) -> Option<Paragraph> {
        self.raw.upgrade().map(Paragraph)
    }
}

impl PartialEq for Weak {
    fn eq(&self, other: &Self) -> bool {
        match (self.raw.upgrade(), other.raw.upgrade()) {
            (Some(p1), Some(p2)) => p1 == p2,
            _ => false,
        }
    }
}
