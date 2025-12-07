//! Text rendering for Vello using Parley.
//!
//! This module provides Vello-native implementations of iced's text traits,
//! using Parley for text layout and Vello for rendering.

mod context;
mod editor;

pub use context::{font_context, layout_context, load_font};
pub use editor::{PlainEditor, PlainEditorDriver, Scroll};

use crate::core::text::editor::{Action, Cursor, Edit, Line, LineEnding, Motion, Position, Selection};
use crate::core::text::{
    Alignment, Difference, Highlighter, Hit, LineHeight, Shaping, Span, Wrapping, highlighter,
};
use crate::core::{Font, Pixels, Point, Rectangle, Size};

use parley::{FontStack, Layout, StyleProperty};
use std::sync::{Arc, OnceLock, RwLock};
use vello::peniko::Brush;

static FONT_NAME_CACHE: OnceLock<RwLock<std::collections::HashMap<String, &'static str>>> =
    OnceLock::new();

/// A paragraph of text that has been laid out for Vello rendering.
#[derive(Clone)]
pub struct Paragraph {
    /// The content of the paragraph.
    pub(crate) content: String,
    /// The Parley layout.
    layout: Option<Arc<Layout<Brush>>>,
    /// The font.
    pub(crate) font: Font,
    /// The font size.
    pub(crate) size: Pixels,
    /// The line height.
    line_height: LineHeight,
    /// The bounds of the paragraph.
    bounds: Size,
    /// The minimum bounds (calculated from layout).
    min_bounds: Size,
    /// Horizontal alignment.
    align_x: Alignment,
    /// Vertical alignment.
    align_y: crate::core::alignment::Vertical,
    /// Text wrapping.
    wrapping: Wrapping,
    /// Text shaping strategy.
    shaping: Shaping,
}

impl std::fmt::Debug for Paragraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paragraph")
            .field("content", &self.content)
            .field("font", &self.font)
            .field("size", &self.size)
            .field("bounds", &self.bounds)
            .finish()
    }
}

impl Paragraph {
    /// Creates a new paragraph.
    pub fn new() -> Self {
        Self {
            content: String::new(),
            layout: None,
            font: Font::default(),
            size: Pixels(16.0),
            line_height: LineHeight::default(),
            bounds: Size::ZERO,
            min_bounds: Size::ZERO,
            align_x: Alignment::Left,
            align_y: crate::core::alignment::Vertical::Top,
            wrapping: Wrapping::default(),
            shaping: Shaping::Basic,
        }
    }

    /// Build the Parley layout for this paragraph and return the min_bounds.
    fn build_layout(&self) -> (Option<Arc<Layout<Brush>>>, Size) {
        if self.content.is_empty() {
            return (None, Size::ZERO);
        }

        let font_ctx = font_context();
        let layout_ctx = layout_context();

        let mut font_ctx = font_ctx.write().unwrap();
        let mut layout_ctx = layout_ctx.write().unwrap();

        // Create a ranged builder
        let mut builder = layout_ctx.ranged_builder(
            &mut font_ctx,
            &self.content,
            1.0,  // display_scale
            true, // quantize to pixel boundaries
        );

        // Set font properties
        builder.push_default(StyleProperty::FontSize(self.size.0));

        // Map iced Font to Parley FontStack
        let font_stack = match &self.font.family {
            crate::core::font::Family::Name(name) => {
                let name_str: &str = name.as_ref();
                FontStack::from(name_str)
            }
            crate::core::font::Family::Serif => FontStack::from("serif"),
            crate::core::font::Family::SansSerif => FontStack::from("sans-serif"),
            crate::core::font::Family::Cursive => FontStack::from("cursive"),
            crate::core::font::Family::Fantasy => FontStack::from("fantasy"),
            crate::core::font::Family::Monospace => FontStack::from("monospace"),
        };
        builder.push_default(font_stack);

        // Set line height
        let line_height = match self.line_height {
            LineHeight::Relative(factor) => parley::LineHeight::FontSizeRelative(factor),
            LineHeight::Absolute(pixels) => parley::LineHeight::Absolute(pixels.0),
        };
        builder.push_default(line_height);

        // Build the layout
        let mut layout = builder.build(&self.content);

        // Break lines with the specified bounds (or None for natural size)
        // This matches iced_graphics behavior - set size first, then measure
        let break_width = if self.bounds.width > 0.0 {
            Some(self.bounds.width)
        } else {
            None
        };
        layout.break_all_lines(break_width);

        // We don't call layout.align() here because alignment is handled during rendering.
        // For fill_paragraph(), widgets use min_bounds to position text correctly.
        // For fill_text(), the renderer adjusts position based on alignment.

        // Calculate min_bounds from the actual layout (after line breaking)
        let width = layout.width();

        // Use visual bounds (ascent + descent) for height, not line_height.
        // This excludes line spacing and matches cosmic-text behavior.
        let height = if let Some(first_line) = layout.lines().next() {
            let metrics = first_line.metrics();
            metrics.ascent + metrics.descent
        } else {
            layout.height()
        };

        let min_bounds = Size::new(width, height);

        (Some(Arc::new(layout)), min_bounds)
    }

    /// Get the layout.
    pub fn layout(&self) -> Option<Arc<Layout<Brush>>> {
        self.layout.clone()
    }

    /// Downgrade to a weak reference.
    pub fn downgrade(&self) -> Weak {
        // TODO: Implement proper weak reference system
        Weak {
            inner: self.clone(),
        }
    }
}

impl Default for Paragraph {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::core::text::Paragraph for Paragraph {
    type Font = Font;

    fn with_text(text: crate::core::Text<&str>) -> Self {
        let mut paragraph = Self::new();
        paragraph.content = text.content.to_string();
        paragraph.font = text.font;
        paragraph.size = text.size;
        paragraph.line_height = text.line_height;
        paragraph.bounds = text.bounds;
        paragraph.align_x = text.align_x;
        paragraph.align_y = text.align_y;
        paragraph.wrapping = text.wrapping;
        paragraph.shaping = text.shaping;

        // Build the layout immediately and store min_bounds
        let (layout, min_bounds) = paragraph.build_layout();
        paragraph.layout = layout;
        paragraph.min_bounds = min_bounds;

        paragraph
    }

    fn with_spans<Link>(text: crate::core::Text<&[Span<'_, Link, Self::Font>]>) -> Self {
        // TODO: Implement rich text spans
        let mut paragraph = Self::new();
        let content: String = text.content.iter().map(|span| span.text.as_ref()).collect();
        paragraph.content = content;
        paragraph.font = text.font;
        paragraph.size = text.size;
        paragraph.line_height = text.line_height;
        paragraph.bounds = text.bounds;
        paragraph.align_x = text.align_x;
        paragraph.align_y = text.align_y;
        paragraph.wrapping = text.wrapping;
        paragraph.shaping = text.shaping;

        // Build the layout immediately and store min_bounds
        let (layout, min_bounds) = paragraph.build_layout();
        paragraph.layout = layout;
        paragraph.min_bounds = min_bounds;

        paragraph
    }

    fn resize(&mut self, new_bounds: Size) {
        if self.bounds != new_bounds {
            self.bounds = new_bounds;

            // Rebuild layout with new bounds and update min_bounds
            let (layout, min_bounds) = self.build_layout();
            self.layout = layout;
            self.min_bounds = min_bounds;
        }
    }

    fn compare(&self, text: crate::core::Text<()>) -> Difference {
        // We compare against the unit text to see if layout needs updating
        if self.size != text.size || self.line_height != text.line_height {
            Difference::Shape
        } else if self.bounds != text.bounds {
            Difference::Bounds
        } else {
            Difference::None
        }
    }

    fn size(&self) -> Pixels {
        self.size
    }

    fn font(&self) -> Self::Font {
        self.font
    }

    fn line_height(&self) -> LineHeight {
        self.line_height
    }

    fn align_x(&self) -> Alignment {
        self.align_x
    }

    fn align_y(&self) -> crate::core::alignment::Vertical {
        self.align_y
    }

    fn wrapping(&self) -> Wrapping {
        self.wrapping
    }

    fn shaping(&self) -> Shaping {
        self.shaping
    }

    fn bounds(&self) -> Size {
        self.bounds
    }

    fn min_bounds(&self) -> Size {
        self.min_bounds
    }

    fn hit_test(&self, point: Point) -> Option<Hit> {
        let layout = self.layout.as_ref()?;

        // Use Parley's cursor_from_point to find the character at this position
        use parley::Cursor as ParleyCursor;
        let cursor = ParleyCursor::from_point(layout, point.x, point.y);

        // Convert byte index to character offset
        let byte_index = cursor.index();
        let char_offset = self.content[..byte_index.min(self.content.len())]
            .chars()
            .count();

        Some(Hit::CharOffset(char_offset))
    }

    fn hit_span(&self, _point: Point) -> Option<usize> {
        // TODO: Implement span hit testing
        None
    }

    fn span_bounds(&self, _index: usize) -> Vec<Rectangle> {
        // TODO: Implement span bounds
        Vec::new()
    }

    fn grapheme_position(&self, line: usize, column: usize) -> Option<Point> {
        let layout = self.layout.as_ref()?;

        // Convert line/column to byte index
        let mut current_line = 0;
        let mut current_column = 0;
        let mut byte_index = 0;

        for (i, ch) in self.content.char_indices() {
            // Check if we've reached the target position
            if current_line == line && current_column == column {
                byte_index = i;
                break;
            }

            // Update position for next iteration
            if ch == '\n' {
                current_line += 1;
                current_column = 0;
            } else {
                current_column += 1;
            }

            // Always update byte_index to point after this character
            byte_index = i + ch.len_utf8();
        }

        // After the loop, byte_index points to the end of the last processed character
        // If we're still on the target line, this is the correct position for the cursor

        // Create a cursor at this position
        use parley::Cursor as ParleyCursor;
        let cursor =
            ParleyCursor::from_byte_index(layout, byte_index, parley::Affinity::Downstream);

        // Get geometry for the cursor
        let geometry = cursor.geometry(layout, 0.0);

        Some(Point::new(geometry.x0 as f32, geometry.y0 as f32))
    }
}

/// A text editor for Vello using our forked PlainEditor with scroll support.
#[derive(Clone)]
pub struct Editor {
    /// Our forked PlainEditor instance with scroll tracking.
    editor: PlainEditor<vello::peniko::Brush>,
    /// The font.
    font: Font,
    /// The bounds (viewport size).
    bounds: Size,
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("font", &self.font)
            .field("bounds", &self.bounds)
            .finish()
    }
}

impl Editor {
    /// Creates a new editor.
    pub fn new() -> Self {
        let mut editor = PlainEditor::new(16.0);

        // Set default line height to 1.3 (iced's default)
        let _ = editor
            .edit_styles()
            .insert(parley::StyleProperty::LineHeight(
                parley::LineHeight::FontSizeRelative(1.3),
            ));

        // Set default font
        let default_font = Font::default();
        let font_stack = Self::font_to_stack(&default_font);
        let _ = editor
            .edit_styles()
            .insert(parley::StyleProperty::FontStack(font_stack));

        Self {
            editor,
            font: default_font,
            bounds: Size::ZERO,
        }
    }

    fn font_to_stack(font: &Font) -> FontStack<'static> {
        match &font.family {
            crate::core::font::Family::Name(name) => {
                let name_str: &str = name.as_ref();

                let cache =
                    FONT_NAME_CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()));

                // Check cache first
                {
                    let read_cache = cache.read().unwrap();
                    if let Some(&cached) = read_cache.get(name_str) {
                        return FontStack::from(cached);
                    }
                }

                // Leak and cache if not found
                let leaked: &'static str = Box::leak(name_str.to_string().into_boxed_str());
                let mut write_cache = cache.write().unwrap();
                let _ = write_cache.insert(name_str.to_string(), leaked);
                FontStack::from(leaked)
            }
            crate::core::font::Family::Serif => FontStack::from("serif"),
            crate::core::font::Family::SansSerif => FontStack::from("sans-serif"),
            crate::core::font::Family::Cursive => FontStack::from("cursive"),
            crate::core::font::Family::Fantasy => FontStack::from("fantasy"),
            crate::core::font::Family::Monospace => FontStack::from("monospace"),
        }
    }

    /// Get the current scroll state.
    pub fn scroll(&self) -> Scroll {
        self.editor.scroll()
    }

    /// Get the current scroll offset in pixels (for backward compatibility).
    pub fn scroll_offset(&self) -> f32 {
        // Calculate total pixel offset from scroll state
        if let Some(layout) = self.editor.try_layout() {
            // Sum up heights of lines before scroll.line, plus vertical offset
            let mut offset = 0.0;
            for (i, line) in layout.lines().enumerate() {
                if i >= self.editor.scroll().line {
                    break;
                }
                offset += line.metrics().line_height;
            }
            offset + self.editor.scroll().vertical
        } else {
            0.0
        }
    }

    /// Scroll by a number of lines (can be fractional).
    ///
    /// This implements smart scrolling following the cosmic-text pattern:
    /// - Accumulates fractional scroll amounts
    /// - Converts to discrete line + continuous vertical offset
    /// - Clamps to valid range
    fn scroll_by_lines(&mut self, delta_lines: f32) {
        let Some(layout) = self.editor.try_layout() else {
            return;
        };

        let lines: Vec<_> = layout.lines().collect();
        if lines.is_empty() {
            return;
        }

        let mut scroll = self.editor.scroll();

        // Estimate line height from current line or first line
        let line_height = if scroll.line < lines.len() {
            lines[scroll.line].metrics().line_height
        } else if !lines.is_empty() {
            lines[0].metrics().line_height
        } else {
            return;
        };

        // Convert line delta to pixels
        let delta_pixels = delta_lines * line_height;

        // Update vertical offset
        scroll.vertical += delta_pixels;

        // Adjust line index based on vertical scroll
        while scroll.vertical < 0.0 && scroll.line > 0 {
            scroll.line -= 1;
            if scroll.line < lines.len() {
                scroll.vertical += lines[scroll.line].metrics().line_height;
            }
        }

        while scroll.line < lines.len() {
            let current_line_height = lines[scroll.line].metrics().line_height;
            if scroll.vertical < current_line_height {
                break;
            }
            scroll.vertical -= current_line_height;
            scroll.line += 1;
        }

        // Clamp to valid range
        if scroll.line >= lines.len() {
            scroll.line = lines.len().saturating_sub(1);
            scroll.vertical = 0.0;
        }

        // Ensure we don't scroll past the bottom
        let total_height = layout.height();
        let viewport_height = self.bounds.height;
        if viewport_height > 0.0 && total_height > viewport_height {
            let current_offset = self.calculate_pixel_offset(&lines, scroll);
            let max_offset = total_height - viewport_height;
            if current_offset > max_offset {
                // Back up to max offset
                scroll = self.pixel_offset_to_scroll(&lines, max_offset);
            }
        }

        // Don't allow scrolling above the top
        if scroll.line == 0 && scroll.vertical < 0.0 {
            scroll.vertical = 0.0;
        }

        self.editor.set_scroll(scroll);
    }

    /// Calculate pixel offset from scroll state.
    fn calculate_pixel_offset(
        &self,
        lines: &[parley::layout::Line<'_, vello::peniko::Brush>],
        scroll: Scroll,
    ) -> f32 {
        let mut offset = 0.0;
        for (i, line) in lines.iter().enumerate() {
            if i >= scroll.line {
                break;
            }
            offset += line.metrics().line_height;
        }
        offset + scroll.vertical
    }

    /// Convert pixel offset to scroll state.
    fn pixel_offset_to_scroll(
        &self,
        lines: &[parley::layout::Line<'_, vello::peniko::Brush>],
        pixel_offset: f32,
    ) -> Scroll {
        let mut remaining = pixel_offset;
        let mut line = 0;

        for (i, layout_line) in lines.iter().enumerate() {
            let line_height = layout_line.metrics().line_height;
            if remaining < line_height {
                return Scroll {
                    line: i,
                    vertical: remaining,
                };
            }
            remaining -= line_height;
            line = i + 1;
        }

        Scroll {
            line,
            vertical: 0.0,
        }
    }

    /// Get a reference to the editor.
    pub fn editor(&self) -> &PlainEditor<vello::peniko::Brush> {
        &self.editor
    }

    /// Get a mutable reference to the editor.
    pub fn editor_mut(&mut self) -> &mut PlainEditor<vello::peniko::Brush> {
        &mut self.editor
    }

    /// Downgrade to a weak reference.
    pub fn downgrade(&self) -> EditorWeak {
        EditorWeak {
            inner: self.clone(),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::core::text::Editor for Editor {
    type Font = Font;

    fn with_text(text: &str) -> Self {
        let mut editor = Self::new();
        editor.editor.set_text(text);
        editor
    }

    fn is_empty(&self) -> bool {
        self.editor.text() == ""
    }

    fn cursor(&self) -> Cursor {
        let focus = self.editor.raw_selection().focus();
        let cursor_index = focus.index();

        // Calculate line and column from byte index
        let text = self.editor.raw_text();
        let mut line = 0;
        let mut line_start = 0;

        for (i, c) in text.char_indices() {
            if i >= cursor_index {
                break;
            }
            if c == '\n' {
                line += 1;
                line_start = i + 1;
            }
        }

        // Column is the number of chars from line start to cursor
        let column = text[line_start..cursor_index.min(text.len())].chars().count();

        let position = Position { line, column };

        // Check if there's a selection
        let raw_selection = self.editor.raw_selection();
        let selection = if raw_selection.is_collapsed() {
            None
        } else {
            // Get the anchor position
            let anchor = raw_selection.anchor();
            let anchor_index = anchor.index();

            let mut anchor_line = 0;
            let mut anchor_line_start = 0;

            for (i, c) in text.char_indices() {
                if i >= anchor_index {
                    break;
                }
                if c == '\n' {
                    anchor_line += 1;
                    anchor_line_start = i + 1;
                }
            }

            let anchor_column = text[anchor_line_start..anchor_index.min(text.len())].chars().count();

            Some(Position {
                line: anchor_line,
                column: anchor_column,
            })
        };

        Cursor { position, selection }
    }

    fn selection(&self) -> Selection {
        if let Some(layout) = self.editor.try_layout() {
            let raw_selection = self.editor.raw_selection();
            let scroll_offset = self.scroll_offset();

            if raw_selection.is_collapsed() {
                let focus = raw_selection.focus();

                // Find the line this cursor is on to get proper line height
                let cursor_line = layout.lines().enumerate().find(|(_, line)| {
                    let line_range = line.text_range();
                    let cursor_idx = focus.index();
                    cursor_idx >= line_range.start && cursor_idx <= line_range.end
                });

                // Use line height for cursor size
                let cursor_size = if let Some((_, line)) = cursor_line {
                    line.metrics().line_height
                } else {
                    16.0 // Fallback to default font size
                };

                let geometry = focus.geometry(layout, cursor_size);

                // Adjust Y position for scroll
                Selection::Caret(Point::new(
                    geometry.x0 as f32,
                    (geometry.y0 as f32) - scroll_offset,
                ))
            } else {
                // Selection rectangles also need scroll adjustment
                let mut rectangles = Vec::new();
                raw_selection.geometry_with(layout, |rect, _level| {
                    rectangles.push(Rectangle::new(
                        Point::new(rect.x0 as f32, (rect.y0 as f32) - scroll_offset),
                        Size::new((rect.x1 - rect.x0) as f32, (rect.y1 - rect.y0) as f32),
                    ));
                });
                Selection::Range(rectangles)
            }
        } else {
            Selection::Caret(Point::ORIGIN)
        }
    }

    fn copy(&self) -> Option<String> {
        let selection = self.editor.raw_selection();
        if !selection.is_collapsed() {
            let range = selection.text_range();
            Some(self.editor.raw_text()[range].to_string())
        } else {
            None
        }
    }

    fn move_to(&mut self, cursor: Cursor) {
        // Convert line/column to byte index
        fn position_to_byte_index(pos: &Position, text: &str) -> usize {
            let mut current_line = 0;
            let mut line_start = 0;

            for (i, c) in text.char_indices() {
                if c == '\n' {
                    if current_line == pos.line {
                        // Found the line, now find the column
                        let mut col = 0;
                        for (j, ch) in text[line_start..i].char_indices() {
                            if col >= pos.column {
                                return line_start + j;
                            }
                            col += 1;
                        }
                        // Column is at or beyond line end
                        return i;
                    }
                    current_line += 1;
                    line_start = i + 1;
                }
            }

            // Handle last line (no trailing newline)
            if current_line == pos.line {
                let mut col = 0;
                for (j, _ch) in text[line_start..].char_indices() {
                    if col >= pos.column {
                        return line_start + j;
                    }
                    col += 1;
                }
            }

            text.len()
        }

        // Calculate byte indices before borrowing editor mutably
        let text = self.editor.raw_text();
        let focus_byte_index = position_to_byte_index(&cursor.position, text);
        let anchor_byte_index = cursor.selection.as_ref().map(|sel| position_to_byte_index(sel, text));

        let font_ctx = font_context();
        let layout_ctx = layout_context();
        let mut font_ctx = font_ctx.write().unwrap();
        let mut layout_ctx = layout_ctx.write().unwrap();

        let mut driver = PlainEditorDriver {
            editor: &mut self.editor,
            font_cx: &mut font_ctx,
            layout_cx: &mut layout_ctx,
        };

        if let Some(anchor_idx) = anchor_byte_index {
            // If there's a selection, move to anchor first, then extend to focus
            driver.move_to_byte(anchor_idx);
            driver.extend_selection_to_byte(focus_byte_index);
        } else {
            // No selection, just move cursor
            driver.move_to_byte(focus_byte_index);
        }
    }

    fn line(&self, index: usize) -> Option<Line<'_>> {
        let text = self.editor.raw_text();
        let mut lines = text.lines();
        lines.nth(index).map(|text| Line {
            text: std::borrow::Cow::Borrowed(text),
            ending: LineEnding::None,
        })
    }

    fn line_count(&self) -> usize {
        self.editor.raw_text().lines().count().max(1)
    }

    fn perform(&mut self, action: Action) {
        // Calculate scroll offset before borrowing editor mutably
        let scroll_offset = match action {
            Action::Click(_) | Action::Drag(_) => self.scroll_offset(),
            _ => 0.0,
        };

        let font_ctx = font_context();
        let layout_ctx = layout_context();

        let mut font_ctx = font_ctx.write().unwrap();
        let mut layout_ctx = layout_ctx.write().unwrap();

        let mut driver = PlainEditorDriver {
            editor: &mut self.editor,
            font_cx: &mut font_ctx,
            layout_cx: &mut layout_ctx,
        };

        match action {
            Action::Edit(edit) => match edit {
                Edit::Insert(c) => {
                    driver.insert_or_replace_selection(&c.to_string());
                }
                Edit::Paste(text) => {
                    driver.insert_or_replace_selection(&text);
                }
                Edit::Enter => {
                    driver.insert_or_replace_selection("\n");
                }
                Edit::Backspace => {
                    driver.backdelete();
                }
                Edit::Delete => {
                    driver.delete();
                }
                Edit::Indent => {
                    driver.insert_or_replace_selection("\t");
                }
                Edit::Unindent => {
                    // TODO: Implement proper unindent
                }
            },
            Action::Move(motion) => match motion {
                Motion::Left => driver.move_left(),
                Motion::Right => driver.move_right(),
                Motion::Up => driver.move_up(),
                Motion::Down => driver.move_down(),
                Motion::WordLeft => driver.move_word_left(),
                Motion::WordRight => driver.move_word_right(),
                Motion::Home => driver.move_to_line_start(),
                Motion::End => driver.move_to_line_end(),
                Motion::PageUp => {
                    for _ in 0..10 {
                        driver.move_up();
                    }
                }
                Motion::PageDown => {
                    for _ in 0..10 {
                        driver.move_down();
                    }
                }
                Motion::DocumentStart => driver.move_to_text_start(),
                Motion::DocumentEnd => driver.move_to_text_end(),
            },
            Action::Select(motion) => match motion {
                Motion::Left => driver.select_left(),
                Motion::Right => driver.select_right(),
                Motion::Up => driver.select_up(),
                Motion::Down => driver.select_down(),
                Motion::WordLeft => driver.select_word_left(),
                Motion::WordRight => driver.select_word_right(),
                Motion::Home => driver.select_to_line_start(),
                Motion::End => driver.select_to_line_end(),
                Motion::PageUp => {
                    for _ in 0..10 {
                        driver.select_up();
                    }
                }
                Motion::PageDown => {
                    for _ in 0..10 {
                        driver.select_down();
                    }
                }
                Motion::DocumentStart => driver.select_to_text_start(),
                Motion::DocumentEnd => driver.select_to_text_end(),
            },
            Action::SelectWord => {
                let selection = driver.editor.raw_selection();
                let focus = selection.focus();
                if let Some(layout) = driver.editor.try_layout() {
                    let geometry = focus.geometry(layout, 0.0);
                    driver.select_word_at_point(geometry.x0 as f32, geometry.y0 as f32);
                }
            }
            Action::SelectLine => {
                let selection = driver.editor.raw_selection();
                let focus = selection.focus();
                if let Some(layout) = driver.editor.try_layout() {
                    let geometry = focus.geometry(layout, 0.0);
                    driver.select_line_at_point(geometry.x0 as f32, geometry.y0 as f32);
                }
            }
            Action::SelectAll => {
                driver.select_all();
            }
            Action::Click(point) => {
                driver.move_to_point(point.x, point.y + scroll_offset);
            }
            Action::Drag(point) => {
                driver.extend_selection_to_point(point.x, point.y + scroll_offset);
            }
            Action::Scroll { lines } => {
                self.scroll_by_lines(lines as f32);
            }
        }
    }

    fn bounds(&self) -> Size {
        self.bounds
    }

    fn min_bounds(&self) -> Size {
        if let Some(layout) = self.editor.try_layout() {
            Size::new(layout.width(), layout.height())
        } else {
            Size::ZERO
        }
    }

    fn update(
        &mut self,
        new_bounds: Size,
        new_font: Self::Font,
        new_size: Pixels,
        new_line_height: LineHeight,
        _new_wrapping: Wrapping,
        _new_highlighter: &mut impl Highlighter,
    ) {
        self.bounds = new_bounds;
        self.font = new_font;

        // Update editor settings
        self.editor.set_width(if new_bounds.width > 0.0 {
            Some(new_bounds.width)
        } else {
            None
        });

        // Update font stack
        let font_stack = Self::font_to_stack(&new_font);
        let _ = self
            .editor
            .edit_styles()
            .insert(parley::StyleProperty::FontStack(font_stack));

        // Update font size in the editor's style
        let _ = self
            .editor
            .edit_styles()
            .insert(parley::StyleProperty::FontSize(new_size.0));

        // Update line height
        let parley_line_height = match new_line_height {
            LineHeight::Relative(factor) => parley::LineHeight::FontSizeRelative(factor),
            LineHeight::Absolute(pixels) => parley::LineHeight::Absolute(pixels.0),
        };
        let _ = self
            .editor
            .edit_styles()
            .insert(parley::StyleProperty::LineHeight(parley_line_height));

        // Refresh layout
        let font_ctx = font_context();
        let layout_ctx = layout_context();
        let mut font_ctx = font_ctx.write().unwrap();
        let mut layout_ctx = layout_ctx.write().unwrap();
        self.editor.refresh_layout(&mut font_ctx, &mut layout_ctx);
    }

    fn highlight<H: Highlighter>(
        &mut self,
        _font: Self::Font,
        _highlighter: &mut H,
        _format_highlight: impl Fn(&H::Highlight) -> highlighter::Format<Self::Font>,
    ) {
        // TODO: Implement syntax highlighting
    }
}

/// A weak reference to a Paragraph.
#[derive(Clone)]
pub struct Weak {
    inner: Paragraph,
}

impl std::fmt::Debug for Weak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Weak").finish()
    }
}

impl Weak {
    /// Upgrade to a strong reference.
    pub fn upgrade(&self) -> Option<Paragraph> {
        Some(self.inner.clone())
    }

    /// Get the layout if available.
    pub fn layout(&self) -> Option<Arc<Layout<Brush>>> {
        self.inner.layout.clone()
    }
}

/// A weak reference to an Editor.
#[derive(Debug, Clone)]
pub struct EditorWeak {
    inner: Editor,
}

impl EditorWeak {
    /// Upgrade to a strong reference.
    pub fn upgrade(&self) -> Option<Editor> {
        Some(self.inner.clone())
    }
}
