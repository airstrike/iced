//! Rich text editor types.
//!
//! Unlike [`super::editor::Editor`], this editor manages per-character and
//! per-paragraph formatting directly — no Highlighter needed.

use crate::text::{LineHeight, Wrapping};
use crate::{Color, Em, Pixels, Point, Rectangle, Size};

use std::ops::Range;

// Re-export the types we share with the regular editor
pub use super::editor::{
    Action, Cursor, Direction, Edit, Line, LineEnding, Motion, Position, Selection,
};

/// Per-character formatting style.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    /// Bold (font weight >= 700).
    pub bold: Option<bool>,
    /// Italic.
    pub italic: Option<bool>,
    /// Underline.
    pub underline: Option<bool>,
    /// Strikethrough.
    pub strikethrough: Option<bool>,
    /// Override font.
    pub font: Option<crate::Font>,
    /// Override font size in logical pixels.
    pub size: Option<f32>,
    /// Override text color.
    pub color: Option<Color>,
    /// Override letter spacing.
    pub letter_spacing: Option<f32>,
}

/// Paragraph-level formatting style.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParagraphStyle {
    /// Character defaults for the paragraph.
    pub style: Style,
    /// Text alignment.
    pub alignment: Option<crate::text::Alignment>,
    /// Spacing after the paragraph in logical pixels.
    pub spacing_after: Option<f32>,
}

/// Geometry of the first visual line of a paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineGeometry {
    /// Y offset from the top of the buffer to the top of this line.
    pub line_top: f32,
    /// Total height of this line (ascent + descent + leading).
    pub line_height: f32,
    /// Y offset from the top of the buffer to the baseline.
    pub baseline_y: f32,
    /// X offset of the line start (margin + alignment).
    pub x_offset: f32,
}

/// A rich text editor — manages text + per-character formatting.
pub trait Editor: Sized + Default {
    /// The font type.
    type Font: Copy + PartialEq + Default;

    /// Creates a new [`Editor`] with the given text.
    fn with_text(text: &str) -> Self;

    /// Returns true if the editor has no contents.
    fn is_empty(&self) -> bool;

    /// Returns the current cursor.
    fn cursor(&self) -> Cursor;

    /// Returns the current selection geometry.
    fn selection(&self) -> Selection;

    /// Returns the selected text, if any.
    fn copy(&self) -> Option<String>;

    /// Returns the text of a given line.
    fn line(&self, index: usize) -> Option<Line<'_>>;

    /// Returns the number of lines.
    fn line_count(&self) -> usize;

    /// Performs an action on the editor.
    fn perform(&mut self, action: Action);

    /// Moves the cursor.
    fn move_to(&mut self, cursor: Cursor);

    /// Returns the current bounds.
    fn bounds(&self) -> Size;

    /// Returns the minimum bounds to fit contents.
    fn min_bounds(&self) -> Size;

    /// Returns the hint factor, if any.
    fn hint_factor(&self) -> Option<f32>;

    /// Updates layout — NO Highlighter parameter.
    fn update(
        &mut self,
        new_bounds: Size,
        new_font: Self::Font,
        new_size: Pixels,
        new_line_height: LineHeight,
        new_letter_spacing: Em,
        new_font_features: Vec<crate::font::Feature>,
        new_font_variations: Vec<crate::font::Variation>,
        new_wrapping: Wrapping,
        new_hint_factor: Option<f32>,
        new_default_style: Style,
    );

    /// Set character formatting on a range.
    fn set_span_style(&mut self, line: usize, range: Range<usize>, style: &Style);

    /// Set paragraph-level defaults + alignment.
    fn set_paragraph_style(&mut self, line: usize, style: &ParagraphStyle);

    /// Set the default alignment for lines without explicit alignment.
    ///
    /// Lines that were previously using the old default are updated to the new
    /// default.  Freshly-created lines (`None` in cosmic-text) also receive it.
    fn align_x(&mut self, alignment: crate::text::Alignment);

    /// Set the left margin for a line (pixels). Creates space for list markers.
    fn set_margin_left(&mut self, line: usize, margin: f32);

    /// First visual line geometry for a paragraph line.
    /// Returns None if the line doesn't exist or isn't laid out.
    fn line_geometry(&self, line: usize) -> Option<LineGeometry>;

    /// Read character formatting at a position.
    fn style_at(&self, line: usize, column: usize) -> Style;

    /// Read paragraph style.
    fn paragraph_style(&self, line: usize) -> ParagraphStyle;
}

/// A renderer that can draw a rich editor.
pub trait Renderer: crate::text::Renderer {
    /// The rich editor type.
    type RichEditor: Editor<Font = <Self as crate::text::Renderer>::Font> + 'static;

    /// Draws the rich editor.
    fn fill_rich_editor(
        &mut self,
        editor: &Self::RichEditor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );
}
