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

/// Paragraph-level formatting types.
pub mod paragraph;
/// Per-character (span) formatting types.
pub mod span;

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
        new_default_style: span::Style,
    );

    /// Set character formatting on a range.
    fn set_span_style(&mut self, line: usize, range: Range<usize>, style: &span::Style);

    /// Set paragraph-level defaults + alignment.
    fn set_paragraph_style(&mut self, line: usize, style: &paragraph::Style);

    /// Set the default alignment for lines without explicit alignment.
    ///
    /// Lines that were previously using the old default are updated to the new
    /// default.  Freshly-created lines (`None` in cosmic-text) also receive it.
    fn align_x(&mut self, alignment: crate::text::Alignment);

    /// Set the left margin for a line (pixels). Creates space for list markers.
    fn set_margin_left(&mut self, line: usize, margin: f32);

    /// First visual line geometry for a paragraph line.
    /// Returns None if the line doesn't exist or isn't laid out.
    fn line_geometry(&self, line: usize) -> Option<paragraph::Geometry>;

    /// Read character formatting at a position.
    fn span_style_at(&self, line: usize, column: usize) -> span::Style;

    /// Read paragraph style.
    fn paragraph_style_at(&self, line: usize) -> paragraph::Style;
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
