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

/// Which decoration line a [`Editor::decorations`] callback rectangle
/// represents. Useful for widgets that want to draw underlines and
/// strikethroughs in different colors, or to opt out of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decoration {
    /// A single underline (the default for `text_decoration.underline`).
    Underline,
    /// Two stacked underlines (for `UnderlineStyle::Double`).
    DoubleUnderline,
    /// A line through the middle of the glyphs.
    Strikethrough,
    /// A line above the glyphs (between ascender and line top).
    Overline,
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

    /// Returns the number of logical pixels the renderer should shift
    /// the buffer's `(0, 0)` *down* from the editor's content-area
    /// origin so the first visible line's ascenders don't clip.
    ///
    /// Non-zero only when the user-chosen line height for the first
    /// line is smaller than the font's natural ascent + descent — at
    /// which point cosmic-text centers glyphs around the baseline and
    /// the topmost ascenders end up above `line_top = 0`. The widget
    /// uses this value to offset the buffer position it passes to
    /// the renderer's `fill_rich_editor`.
    ///
    /// Default impl returns `0.0` (no overflow) for backends without
    /// glyph-extent awareness.
    fn visual_top_pad(&self) -> f32 {
        0.0
    }

    /// Returns the number of logical pixels the LAST visible line's
    /// glyph bottom extends below its slot bottom — the bottom-side
    /// counterpart of [`visual_top_pad`].
    ///
    /// Widgets whose allocation matches `min_bounds().height` already
    /// reserve room for this overflow (it's baked into the measure).
    /// Widgets that constrain the buffer to a fixed viewport (and
    /// hence may render glyphs whose descenders extend past their
    /// clip rect) can read this to extend the clip on the bottom.
    ///
    /// Default impl returns `0.0` for backends without glyph-extent
    /// awareness.
    fn visual_bottom_pad(&self) -> f32 {
        0.0
    }

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

    /// Set vertical margins for a paragraph line.
    fn set_paragraph_spacing(&mut self, line: usize, top: f32, bottom: f32);

    /// First visual line geometry for a paragraph line.
    /// Returns None if the line doesn't exist or isn't laid out.
    fn line_geometry(&self, line: usize) -> Option<paragraph::Geometry>;

    /// Calls `f` with the pixel rectangle of each visual line segment
    /// within the given character range on `line`.
    ///
    /// Coordinates match `selection()` — already scaled by hint_factor.
    /// Zero-width ranges produce no callbacks.
    fn highlight_rect(
        &self,
        _line: usize,
        _from: usize,
        _to: usize,
        _f: &mut dyn FnMut(Rectangle),
    ) {
    }

    /// Calls `f` once per decoration span (underline, strikethrough,
    /// overline) currently in the buffer, with the rectangle to fill
    /// and the resolved color.
    ///
    /// Coordinates match `selection()` and `highlight_rect()` —
    /// already scaled by `hint_factor`, ready for the widget to add
    /// its buffer-origin offset and pass to `fill_quad`.
    ///
    /// `default_color` is the editor's text color, used when neither
    /// the span nor the decoration carries its own color override.
    ///
    /// Default impl is a no-op for backends without decoration data.
    fn decorations(&self, _default_color: Color, _f: &mut dyn FnMut(Decoration, Rectangle, Color)) {
    }

    /// Read character formatting at a position.
    fn span_style_at(&self, line: usize, column: usize) -> span::Style;

    /// Read paragraph style.
    fn paragraph_style_at(&self, line: usize) -> paragraph::Style;

    /// Enable or disable automatic scrolling to keep the cursor visible.
    fn set_scrollable(&mut self, scrollable: bool);
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
