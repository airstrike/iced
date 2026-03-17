//! Rich text editor — adapted from `text/editor.rs` without Highlighter.
//!
//! # TODO: Font name lifetime strategy
//!
//! iced's `font::Family::Name(&'static str)` requires a `'static` lifetime,
//! but cosmic-text copies font names into `SmolStr` (via `FamilyOwned`).
//! When reading attrs back from cosmic-text, we get `Family::Name(&str)`
//! borrowed from the SmolStr — NOT the original `&'static str`.
//!
//! We explored several approaches:
//!
//! - **`Cow<'static, str>`**: Zero-cost for the forward path (`Borrowed`),
//!   allocates once on readback (`Owned`). But `Cow` isn't `Copy`, so `Font`
//!   and `Family` would lose `Copy`, rippling through all of iced.
//!
//! - **`Arc<str>`**: Same Copy problem as `Cow`, and allocates in both
//!   directions (ref-counted).
//!
//! - **Global string interning** (`Mutex<HashSet<&'static str>>`): Keeps
//!   `&'static str` and `Copy`, but requires locking on every readback.
//!
//! - **Local interner in `Internal`** (current approach): A `HashSet<&'static str>`
//!   on `Internal` populated on the write path (`set_span_style`,
//!   `set_paragraph_style`, default font). The read path (`style_at`,
//!   `paragraph_style`) looks up names without mutation or locking. If a name
//!   is missing, we panic — every font name entering through iced's API is
//!   `&'static str` and should have been registered.
//!
//! The long-term fix is likely changing `Family::Name` to `Cow<'static, str>`
//! and accepting the loss of `Copy` on `Font`, but that's a large cross-crate
//! refactor best done in a dedicated PR.
use crate::core::font;
use crate::core::text::editor::{
    self, Action, Cursor, Direction, Edit, Motion, Position, Selection,
};
use crate::core::text::rich_editor::{self, paragraph, span::Style};
use crate::core::text::{Alignment, LineHeight, Wrapping};
use crate::core::{Color, Em, Font, Pixels, Point, Rectangle, Size};
use crate::text;

use cosmic_text::Edit as _;

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::ops::Range;
use std::sync::{self, Arc, RwLock};

/// A rich text editor.
#[derive(Debug, PartialEq)]
pub struct Editor(Option<Arc<Internal>>);

struct Internal {
    document: cosmic_text::Editor<'static>,
    selection: RwLock<Option<Selection>>,
    font: Font,
    bounds: Size,
    hint: bool,
    hint_factor: f32,
    version: text::Version,
    letter_spacing: Em,
    font_features: Vec<font::Feature>,
    font_variations: Vec<font::Variation>,
    line_height_ratio: f32,
    default_alignment: Alignment,
    default_style: Style,
    /// Every `&'static str` font name that has entered the editor.
    /// See module-level doc for the full rationale.
    font_names: HashSet<&'static str>,
}

impl Editor {
    /// Creates a new empty [`Editor`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the buffer of the [`Editor`].
    pub fn buffer(&self) -> &cosmic_text::Buffer {
        buffer_from_editor(&self.internal().document)
    }

    /// Creates a [`Weak`] reference to the [`Editor`].
    ///
    /// This is useful to avoid cloning the [`Editor`] when
    /// referential guarantees are unnecessary. For instance,
    /// when creating a rendering tree.
    pub fn downgrade(&self) -> Weak {
        let editor = self.internal();

        Weak {
            raw: Arc::downgrade(editor),
            bounds: editor.bounds,
        }
    }

    fn internal(&self) -> &Arc<Internal> {
        self.0
            .as_ref()
            .expect("Rich editor should always be initialized")
    }

    fn with_internal_mut<T>(&mut self, f: impl FnOnce(&mut Internal) -> T) -> T {
        let editor = self
            .0
            .take()
            .expect("Rich editor should always be initialized");

        // TODO: Handle multiple strong references somehow
        let mut internal =
            Arc::try_unwrap(editor).expect("Rich editor cannot have multiple strong references");

        // Clear cursor cache
        let _ = internal
            .selection
            .write()
            .expect("Write to cursor cache")
            .take();

        let result = f(&mut internal);

        self.0 = Some(Arc::new(internal));

        result
    }
}

impl rich_editor::Editor for Editor {
    type Font = Font;

    fn with_text(text: &str) -> Self {
        let mut buffer = cosmic_text::Buffer::new_empty(cosmic_text::Metrics {
            font_size: 1.0,
            line_height: 1.0,
        });

        let mut font_system = text::font_system().write().expect("Write font system");

        buffer.set_text(
            font_system.raw(),
            text,
            &cosmic_text::Attrs::new(),
            cosmic_text::Shaping::Advanced,
            None,
        );

        Editor(Some(Arc::new(Internal {
            document: cosmic_text::Editor::new(buffer),
            version: font_system.version(),
            ..Default::default()
        })))
    }

    fn is_empty(&self) -> bool {
        let buffer = self.buffer();

        buffer.lines.is_empty() || (buffer.lines.len() == 1 && buffer.lines[0].text().is_empty())
    }

    fn line(&self, index: usize) -> Option<editor::Line<'_>> {
        self.buffer().lines.get(index).map(|line| editor::Line {
            text: Cow::Borrowed(line.text()),
            ending: match line.ending() {
                cosmic_text::LineEnding::Lf => editor::LineEnding::Lf,
                cosmic_text::LineEnding::CrLf => editor::LineEnding::CrLf,
                cosmic_text::LineEnding::Cr => editor::LineEnding::Cr,
                cosmic_text::LineEnding::LfCr => editor::LineEnding::LfCr,
                cosmic_text::LineEnding::None => editor::LineEnding::None,
            },
        })
    }

    fn line_count(&self) -> usize {
        self.buffer().lines.len()
    }

    fn copy(&self) -> Option<String> {
        self.internal().document.copy_selection()
    }

    fn selection(&self) -> editor::Selection {
        let internal = self.internal();

        if let Ok(Some(cursor)) = internal.selection.read().as_deref() {
            return cursor.clone();
        }

        let cursor = internal.document.cursor();
        let buffer = buffer_from_editor(&internal.document);

        let cursor = match internal.document.selection_bounds() {
            Some((start, end)) => {
                let start_cursor = cosmic_text::Cursor::new(start.line, start.index);
                let end_cursor = cosmic_text::Cursor::new(end.line, end.index);

                let regions = buffer
                    .layout_runs()
                    .filter_map(|run| {
                        // Empty lines within the selection range still need a
                        // visible indicator so the highlight looks continuous.
                        if run.glyphs.is_empty()
                            && run.line_i >= start_cursor.line
                            && run.line_i <= end_cursor.line
                        {
                            let w = run.line_height * 0.3;
                            let x = empty_line_x(run.x_offset, w, &buffer.lines, run.line_i);
                            return Some(
                                Rectangle {
                                    x,
                                    width: w,
                                    y: run.line_top,
                                    height: run.line_height,
                                } * (1.0 / internal.hint_factor),
                            );
                        }

                        let (x, width) = run.highlight(start_cursor, end_cursor)?;
                        if width > 0.0 {
                            Some(
                                Rectangle {
                                    x,
                                    width,
                                    y: run.line_top,
                                    height: run.line_height,
                                } * (1.0 / internal.hint_factor),
                            )
                        } else {
                            None
                        }
                    })
                    .collect();

                Selection::Range(regions)
            }
            _ => {
                let (caret_x, caret_y, caret_h) = caret_position(cursor, buffer);
                let f = 1.0 / internal.hint_factor;

                // Keep the 1px-wide caret within the editor bounds so it
                // isn't clipped on right-aligned (or end-of-line) text.
                let x = (caret_x * f).min(internal.bounds.width - 1.0).max(0.0);

                Selection::Caret(Rectangle::new(
                    Point::new(x, caret_y * f),
                    Size::new(1.0, caret_h * f),
                ))
            }
        };

        *internal.selection.write().expect("Write to cursor cache") = Some(cursor.clone());

        cursor
    }

    fn cursor(&self) -> Cursor {
        let editor = &self.internal().document;
        let cursor_pos = editor.cursor();

        // Use selection_bounds() to get the actual selected range.
        // This is important for Word/Line selections where the raw
        // Selection variant stores the click point, not the boundaries.
        match editor.selection_bounds() {
            Some((start, end)) => {
                let start_pos = Position {
                    line: start.line,
                    column: start.index,
                };
                let end_pos = Position {
                    line: end.line,
                    column: end.index,
                };

                // Place position at whichever bound the cursor is closest to
                if cursor_pos.line > end.line
                    || (cursor_pos.line == end.line && cursor_pos.index >= end.index)
                {
                    Cursor {
                        position: end_pos,
                        selection: Some(start_pos),
                    }
                } else {
                    Cursor {
                        position: start_pos,
                        selection: Some(end_pos),
                    }
                }
            }
            None => Cursor {
                position: Position {
                    line: cursor_pos.line,
                    column: cursor_pos.index,
                },
                selection: None,
            },
        }
    }

    fn perform(&mut self, action: Action) {
        let mut font_system = text::font_system().write().expect("Write font system");

        self.with_internal_mut(|internal| {
            let editor = &mut internal.document;

            match action {
                // Motion events
                Action::Move(motion) => {
                    if let Some((start, end)) = editor.selection_bounds() {
                        editor.set_selection(cosmic_text::Selection::None);

                        match motion {
                            Motion::Home
                            | Motion::End
                            | Motion::DocumentStart
                            | Motion::DocumentEnd => {
                                editor.action(
                                    font_system.raw(),
                                    cosmic_text::Action::Motion(to_motion(motion)),
                                );
                            }
                            _ => editor.set_cursor(match motion.direction() {
                                Direction::Left => start,
                                Direction::Right => end,
                            }),
                        }
                    } else {
                        editor.action(
                            font_system.raw(),
                            cosmic_text::Action::Motion(to_motion(motion)),
                        );
                    }
                }

                // Selection events
                Action::Select(motion) => {
                    let cursor = editor.cursor();

                    if editor.selection_bounds().is_none() {
                        editor.set_selection(cosmic_text::Selection::Normal(cursor));
                    }

                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Motion(to_motion(motion)),
                    );

                    // Deselect if selection matches cursor position
                    if let Some((start, end)) = editor.selection_bounds()
                        && start.line == end.line
                        && start.index == end.index
                    {
                        editor.set_selection(cosmic_text::Selection::None);
                    }
                }
                Action::SelectWord => {
                    let cursor = editor.cursor();

                    editor.set_selection(cosmic_text::Selection::Word(cursor));
                }
                Action::SelectLine => {
                    let cursor = editor.cursor();

                    editor.set_selection(cosmic_text::Selection::Line(cursor));
                }
                Action::SelectAll => {
                    let buffer = buffer_from_editor(editor);

                    if buffer.lines.len() > 1
                        || buffer
                            .lines
                            .first()
                            .is_some_and(|line| !line.text().is_empty())
                    {
                        let cursor = editor.cursor();

                        editor.set_selection(cosmic_text::Selection::Normal(cosmic_text::Cursor {
                            line: 0,
                            index: 0,
                            ..cursor
                        }));

                        editor.action(
                            font_system.raw(),
                            cosmic_text::Action::Motion(cosmic_text::Motion::BufferEnd),
                        );
                    }
                }

                // Editing events — no topmost_line_changed tracking
                Action::Edit(edit) => match edit {
                    Edit::Insert(c) => {
                        editor.action(font_system.raw(), cosmic_text::Action::Insert(c));
                    }
                    Edit::Paste(text) => {
                        editor.insert_string(&text, None);
                    }
                    Edit::Indent => {
                        editor.action(font_system.raw(), cosmic_text::Action::Indent);
                    }
                    Edit::Unindent => {
                        editor.action(font_system.raw(), cosmic_text::Action::Unindent);
                    }
                    Edit::Enter => {
                        editor.action(font_system.raw(), cosmic_text::Action::Enter);
                    }
                    Edit::Backspace => {
                        editor.action(font_system.raw(), cosmic_text::Action::Backspace);
                    }
                    Edit::Delete => {
                        editor.action(font_system.raw(), cosmic_text::Action::Delete);
                    }
                },

                // Mouse events
                Action::Click(position) => {
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Click {
                            x: (position.x * internal.hint_factor) as i32,
                            y: (position.y * internal.hint_factor) as i32,
                        },
                    );
                }
                Action::Drag(position) => {
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Drag {
                            x: (position.x * internal.hint_factor) as i32,
                            y: (position.y * internal.hint_factor) as i32,
                        },
                    );

                    // Deselect if selection matches cursor position
                    if let Some((start, end)) = editor.selection_bounds()
                        && start.line == end.line
                        && start.index == end.index
                    {
                        editor.set_selection(cosmic_text::Selection::None);
                    }
                }
                Action::Scroll { lines } => {
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Scroll {
                            pixels: lines as f32 * buffer_from_editor(editor).metrics().line_height,
                        },
                    );
                }
            }
        });
    }

    fn move_to(&mut self, cursor: Cursor) {
        self.with_internal_mut(|internal| {
            // TODO: Expose `Affinity`
            internal.document.set_cursor(cosmic_text::Cursor {
                line: cursor.position.line,
                index: cursor.position.column,
                affinity: cosmic_text::Affinity::Before,
            });

            if let Some(selection) = cursor.selection {
                internal
                    .document
                    .set_selection(cosmic_text::Selection::Normal(cosmic_text::Cursor {
                        line: selection.line,
                        index: selection.column,
                        affinity: cosmic_text::Affinity::Before,
                    }));
            }
        });
    }

    fn bounds(&self) -> Size {
        self.internal().bounds
    }

    fn min_bounds(&self) -> Size {
        let internal = self.internal();

        let (bounds, _has_rtl) = text::measure(buffer_from_editor(&internal.document));

        bounds * (1.0 / internal.hint_factor)
    }

    fn hint_factor(&self) -> Option<f32> {
        let internal = self.internal();

        internal.hint.then_some(internal.hint_factor)
    }

    fn update(
        &mut self,
        new_bounds: Size,
        new_font: Font,
        new_size: Pixels,
        new_line_height: LineHeight,
        new_letter_spacing: Em,
        new_font_features: Vec<font::Feature>,
        new_font_variations: Vec<font::Variation>,
        new_wrapping: Wrapping,
        new_hint_factor: Option<f32>,
        new_default_style: Style,
    ) {
        self.with_internal_mut(|internal| {
            let mut font_system = text::font_system().write().expect("Write font system");

            let buffer = buffer_mut_from_editor(&mut internal.document);

            if font_system.version() != internal.version {
                log::trace!("Updating `FontSystem` of rich `Editor`...");

                for line in buffer.lines.iter_mut() {
                    line.reset();
                }

                internal.version = font_system.version();
            }

            // Unlike the regular editor, we do NOT reset AttrsList when
            // font/letter_spacing/features change — that would destroy
            // all rich formatting. Just update the stored values.
            if new_font != internal.font
                || new_letter_spacing != internal.letter_spacing
                || new_font_features != internal.font_features
                || new_font_variations != internal.font_variations
            {
                internal.font = new_font;
                if let font::Family::Name(name) = new_font.family {
                    let _ = internal.font_names.insert(name);
                }
                internal.letter_spacing = new_letter_spacing;
                internal.font_features = new_font_features;
                internal.font_variations = new_font_variations;
            }

            if new_default_style != internal.default_style {
                internal.default_style = new_default_style;

                let base_attrs = text::to_attributes(
                    internal.font,
                    internal.letter_spacing,
                    &internal.font_features,
                    &internal.font_variations,
                );
                let default_attrs = style_to_attrs(
                    &internal.default_style,
                    &base_attrs,
                    internal.line_height_ratio,
                );

                for line in buffer.lines.iter_mut() {
                    let old_list = line.attrs_list();
                    let mut new_list = cosmic_text::AttrsList::new(&default_attrs);
                    for (range, span_attrs) in old_list.spans() {
                        new_list.add_span(range.clone(), &span_attrs.as_attrs());
                    }
                    let _ = line.set_attrs_list(new_list);
                }
            }

            let metrics = buffer.metrics();
            let new_line_height = new_line_height.to_absolute(new_size);
            let mut hinting_changed = false;

            let new_hint_factor = text::hint_factor(new_size, new_hint_factor);

            if new_hint_factor != internal.hint.then_some(internal.hint_factor) {
                internal.hint = new_hint_factor.is_some();
                internal.hint_factor = new_hint_factor.unwrap_or(1.0);

                buffer.set_hinting(
                    font_system.raw(),
                    if internal.hint {
                        cosmic_text::Hinting::Enabled
                    } else {
                        cosmic_text::Hinting::Disabled
                    },
                );

                hinting_changed = true;
            }

            if new_size.0 != metrics.font_size
                || new_line_height.0 != metrics.line_height
                || hinting_changed
            {
                log::trace!("Updating `Metrics` of rich `Editor`...");

                buffer.set_metrics(
                    font_system.raw(),
                    cosmic_text::Metrics::new(
                        new_size.0 * internal.hint_factor,
                        new_line_height.0 * internal.hint_factor,
                    ),
                );
            }

            // Track line_height_ratio for formatting reference
            internal.line_height_ratio = new_line_height.0 / new_size.0;

            let new_wrap = text::to_wrap(new_wrapping);

            if new_wrap != buffer.wrap() {
                log::trace!("Updating `Wrap` strategy of rich `Editor`...");

                buffer.set_wrap(font_system.raw(), new_wrap);
            }

            if new_bounds != internal.bounds || hinting_changed {
                log::trace!("Updating size of rich `Editor`...");

                buffer.set_size(
                    font_system.raw(),
                    Some(new_bounds.width * internal.hint_factor),
                    Some(new_bounds.height * internal.hint_factor),
                );

                internal.bounds = new_bounds;
            }

            internal.document.shape_as_needed(font_system.raw(), false);
        });
    }

    fn set_span_style(&mut self, line: usize, range: Range<usize>, style: &Style) {
        self.with_internal_mut(|internal| {
            if let Some(font) = style.font {
                internal.register_font(font);
            }
            let buffer = buffer_mut_from_editor(&mut internal.document);
            if let Some(buffer_line) = buffer.lines.get_mut(line) {
                let base = buffer_line.attrs_list().defaults();
                let attrs = style_to_attrs(style, &base, internal.line_height_ratio);

                if buffer_line.text().is_empty() {
                    // Empty lines have no characters to span. Update the
                    // line defaults so the style applies to future input
                    // and is visible in `style_at` / cursor queries.
                    let mut new_list = cosmic_text::AttrsList::new(&attrs);
                    // Re-add any existing spans (shouldn't be any, but be safe)
                    for (range, span_attrs) in buffer_line.attrs_list().spans() {
                        new_list.add_span(range.clone(), &span_attrs.as_attrs());
                    }
                    let _ = buffer_line.set_attrs_list(new_list);
                } else {
                    let mut new_list = buffer_line.attrs_list().clone();
                    new_list.add_span(range, &attrs);
                    let _ = buffer_line.set_attrs_list(new_list);
                }
            }
        });
    }

    fn set_paragraph_style(&mut self, line: usize, style: &paragraph::Style) {
        self.with_internal_mut(|internal| {
            if let Some(font) = style.style.font {
                internal.register_font(font);
            }
            let buffer = buffer_mut_from_editor(&mut internal.document);

            // Read buffer font_size (unhinted) before taking a mutable borrow on the line.
            let buffer_font_size = buffer.metrics().font_size
                / if internal.hint {
                    internal.hint_factor
                } else {
                    1.0
                };

            let lh_ratio = style
                .line_height
                .map(|lh: LineHeight| lh.to_absolute(Pixels(buffer_font_size)).0 / buffer_font_size)
                .unwrap_or(internal.line_height_ratio);

            if let Some(buffer_line) = buffer.lines.get_mut(line) {
                // Set default attrs for the line
                let base_attrs = text::to_attributes(
                    internal.font,
                    internal.letter_spacing,
                    &internal.font_features,
                    &internal.font_variations,
                );
                let mut defaults = style_to_attrs(&style.style, &base_attrs, lh_ratio);

                // If line_height is set but size is not, we still need per-line
                // metrics so cosmic-text uses the paragraph's line_height.
                if style.line_height.is_some() && style.style.size.is_none() {
                    defaults = defaults.metrics(cosmic_text::Metrics::new(
                        buffer_font_size,
                        buffer_font_size * lh_ratio,
                    ));
                }

                // Rebuild AttrsList with new defaults, preserving spans
                let old_spans: Vec<_> = buffer_line
                    .attrs_list()
                    .spans()
                    .into_iter()
                    .map(|(range, attrs)| (range.clone(), attrs.as_attrs()))
                    .collect();
                let mut new_list = cosmic_text::AttrsList::new(&defaults);
                for (range, attrs) in &old_spans {
                    new_list.add_span(range.clone(), attrs);
                }
                let _ = buffer_line.set_attrs_list(new_list);

                // Set alignment
                if let Some(align) = style.alignment {
                    let _ = buffer_line.set_align(text::to_align(align));
                }
            }
        });
    }

    fn align_x(&mut self, alignment: Alignment) {
        self.with_internal_mut(|internal| {
            let old = internal.default_alignment;
            let new_align = text::to_align(alignment);
            let buffer = buffer_mut_from_editor(&mut internal.document);

            if alignment != old {
                let old_align = text::to_align(old);
                for line in buffer.lines.iter_mut() {
                    let current = line.align();
                    if current == old_align || current.is_none() {
                        let _ = line.set_align(new_align);
                    }
                }
                internal.default_alignment = alignment;
            } else if new_align.is_some() {
                // Default unchanged, but ensure newly-created lines get it.
                for line in buffer.lines.iter_mut() {
                    if line.align().is_none() {
                        let _ = line.set_align(new_align);
                    }
                }
            }
        });
    }

    fn set_margin_left(&mut self, line: usize, margin: f32) {
        self.with_internal_mut(|internal| {
            let buffer = buffer_mut_from_editor(&mut internal.document);
            if let Some(buffer_line) = buffer.lines.get_mut(line) {
                let _ = buffer_line.set_margin_left(margin);
            }
        });
    }

    fn line_geometry(&self, line: usize) -> Option<rich_editor::paragraph::Geometry> {
        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.document);
        for run in buffer.layout_runs() {
            if run.line_i == line {
                return Some(rich_editor::paragraph::Geometry {
                    line_top: run.line_top,
                    line_height: run.line_height,
                    baseline_y: run.line_y,
                    x_offset: run.x_offset,
                });
            }
            if run.line_i > line {
                break;
            }
        }
        None
    }

    fn span_style_at(&self, line: usize, column: usize) -> Style {
        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.document);
        // Compare against global defaults so per-line custom attrs
        // (e.g. font set on an empty paragraph) are always reported
        // as explicit, not swallowed by a same-as-line-defaults diff.
        let global_defaults = cosmic_text::Attrs::new();
        buffer
            .lines
            .get(line)
            .map(|bl| {
                let span = if bl.text().is_empty() {
                    bl.attrs_list().defaults()
                } else {
                    let idx = if column > 0 && column >= bl.text().len() {
                        column - 1
                    } else {
                        column
                    };
                    bl.attrs_list().get_span(idx)
                };
                attrs_to_style(&span, &global_defaults, &internal.font_names)
            })
            .unwrap_or_default()
    }

    fn paragraph_style_at(&self, line: usize) -> paragraph::Style {
        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.document);
        buffer
            .lines
            .get(line)
            .map(|bl| {
                let defaults = bl.attrs_list().defaults();

                // Extract per-paragraph line_height from metrics_opt.
                let line_height = defaults.metrics_opt.map(|m| {
                    let m: cosmic_text::Metrics = m.into();
                    if m.font_size > 0.0 {
                        LineHeight::Relative(m.line_height / m.font_size)
                    } else {
                        LineHeight::Relative(internal.line_height_ratio)
                    }
                });

                paragraph::Style {
                    style: attrs_to_style(&defaults, &defaults, &internal.font_names),
                    alignment: bl.align().map(|a| match a {
                        cosmic_text::Align::Left => Alignment::Left,
                        cosmic_text::Align::Center => Alignment::Center,
                        cosmic_text::Align::Right => Alignment::Right,
                        cosmic_text::Align::Justified => Alignment::Justified,
                        cosmic_text::Align::End => Alignment::Default,
                    }),
                    spacing_after: None,
                    line_height,
                    ..Default::default()
                }
            })
            .unwrap_or_default()
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self(Some(Arc::new(Internal::default())))
    }
}

impl PartialEq for Internal {
    fn eq(&self, other: &Self) -> bool {
        self.font == other.font
            && self.bounds == other.bounds
            && buffer_from_editor(&self.document).metrics()
                == buffer_from_editor(&other.document).metrics()
    }
}

impl Default for Internal {
    fn default() -> Self {
        Self {
            document: cosmic_text::Editor::new(cosmic_text::Buffer::new_empty(
                cosmic_text::Metrics {
                    font_size: 1.0,
                    line_height: 1.0,
                },
            )),
            selection: RwLock::new(None),
            font: Font::default(),
            bounds: Size::ZERO,
            hint: false,
            hint_factor: 1.0,
            version: text::Version::default(),
            letter_spacing: Em::ZERO,
            font_features: Vec::new(),
            font_variations: Vec::new(),
            line_height_ratio: 1.3,
            default_alignment: Alignment::Default,
            default_style: Style::default(),
            font_names: HashSet::new(),
        }
    }
}

impl Internal {
    fn register_font(&mut self, font: Font) {
        if let font::Family::Name(name) = font.family {
            let _ = self.font_names.insert(name);
        }
    }
}

impl fmt::Debug for Internal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Internal")
            .field("font", &self.font)
            .field("bounds", &self.bounds)
            .finish()
    }
}

/// A weak reference to an [`Editor`].
#[derive(Debug, Clone)]
pub struct Weak {
    raw: sync::Weak<Internal>,
    /// The bounds of the [`Editor`].
    pub bounds: Size,
}

impl Weak {
    /// Tries to update the reference into an [`Editor`].
    pub fn upgrade(&self) -> Option<Editor> {
        self.raw.upgrade().map(Some).map(Editor)
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

fn style_to_attrs<'a>(
    style: &Style,
    base: &cosmic_text::Attrs<'a>,
    line_height_ratio: f32,
) -> cosmic_text::Attrs<'a> {
    let mut attrs = base.clone();

    if let Some(bold) = style.bold {
        attrs = attrs.weight(if bold {
            cosmic_text::Weight::BOLD
        } else {
            cosmic_text::Weight::NORMAL
        });
    }

    if let Some(italic) = style.italic {
        attrs = attrs.style(if italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        });
    }

    if let Some(underline) = style.underline {
        attrs.text_decoration.underline = if underline {
            cosmic_text::UnderlineStyle::Single
        } else {
            cosmic_text::UnderlineStyle::None
        };
    }

    if let Some(strikethrough) = style.strikethrough {
        attrs.text_decoration.strikethrough = strikethrough;
    }

    if let Some(size) = style.size {
        attrs = attrs.metrics(cosmic_text::Metrics::new(size, size * line_height_ratio));
    }

    if let Some(color) = style.color {
        attrs = attrs.color(text::to_color(color));
    }

    if let Some(ls) = style.letter_spacing {
        attrs = attrs.letter_spacing(ls);
    }

    if let Some(opsz) = style.optical_size {
        attrs = attrs.optical_size(match opsz {
            font::OpticalSize::Auto => cosmic_text::OpticalSize::Auto,
            font::OpticalSize::Fixed(bits) => cosmic_text::OpticalSize::Fixed(f32::from_bits(bits)),
            font::OpticalSize::None => cosmic_text::OpticalSize::None,
        });
    }

    if let Some(font) = style.font {
        // Build new attrs from the font, then re-apply non-font style
        let mut font_attrs = text::to_attributes(font, Em::ZERO, &[], &[]);

        // Preserve bold/italic/size/color from the style
        if let Some(bold) = style.bold {
            font_attrs = font_attrs.weight(if bold {
                cosmic_text::Weight::BOLD
            } else {
                cosmic_text::Weight::NORMAL
            });
        } else {
            font_attrs = font_attrs.weight(attrs.weight);
        }

        if let Some(italic) = style.italic {
            font_attrs = font_attrs.style(if italic {
                cosmic_text::Style::Italic
            } else {
                cosmic_text::Style::Normal
            });
        } else {
            font_attrs = font_attrs.style(attrs.style);
        }

        if let Some(size) = style.size {
            font_attrs =
                font_attrs.metrics(cosmic_text::Metrics::new(size, size * line_height_ratio));
        } else if let Some(m) = attrs.metrics_opt {
            let m: cosmic_text::Metrics = m.into();
            font_attrs = font_attrs.metrics(cosmic_text::Metrics::new(m.font_size, m.line_height));
        }

        if let Some(color) = style.color {
            font_attrs = font_attrs.color(text::to_color(color));
        } else if let Some(c) = attrs.color_opt {
            font_attrs.color_opt = Some(c);
        }

        font_attrs.text_decoration = attrs.text_decoration;
        font_attrs.letter_spacing_opt = attrs.letter_spacing_opt;
        font_attrs.optical_size = attrs.optical_size;

        attrs = font_attrs;
    }

    attrs
}

fn attrs_to_style(
    attrs: &cosmic_text::Attrs<'_>,
    defaults: &cosmic_text::Attrs<'_>,
    font_names: &HashSet<&'static str>,
) -> Style {
    // Only report an explicit font when the span differs from the line defaults.
    let font = if attrs.family != defaults.family {
        Some(Font {
            family: from_family(attrs.family, font_names),
            ..Font::default()
        })
    } else {
        None
    };

    Style {
        bold: Some(attrs.weight >= cosmic_text::Weight::BOLD),
        italic: Some(attrs.style == cosmic_text::Style::Italic),
        underline: Some(attrs.text_decoration.underline != cosmic_text::UnderlineStyle::None),
        strikethrough: Some(attrs.text_decoration.strikethrough),
        color: attrs
            .color_opt
            .map(|c| Color::from_rgba8(c.r(), c.g(), c.b(), c.a() as f32 / 255.0)),
        letter_spacing: if attrs.letter_spacing_opt != defaults.letter_spacing_opt {
            attrs.letter_spacing_opt.map(|ls| ls.0)
        } else {
            None
        },
        size: attrs.metrics_opt.map(|m| {
            let m: cosmic_text::Metrics = m.into();
            m.font_size
        }),
        font,
        optical_size: if attrs.optical_size != defaults.optical_size {
            Some(match attrs.optical_size {
                cosmic_text::OpticalSize::Auto => font::OpticalSize::Auto,
                cosmic_text::OpticalSize::Fixed(v) => font::OpticalSize::Fixed(v.to_bits()),
                cosmic_text::OpticalSize::None => font::OpticalSize::None,
            })
        } else {
            None
        },
    }
}

fn from_family(
    family: cosmic_text::Family<'_>,
    font_names: &HashSet<&'static str>,
) -> font::Family {
    match family {
        cosmic_text::Family::Name(name) => {
            font::Family::Name(font_names.get(name).copied().expect(
                "Font name must have been registered via set_span_style or set_paragraph_style",
            ))
        }
        cosmic_text::Family::SansSerif => font::Family::SansSerif,
        cosmic_text::Family::Serif => font::Family::Serif,
        cosmic_text::Family::Cursive => font::Family::Cursive,
        cosmic_text::Family::Fantasy => font::Family::Fantasy,
        cosmic_text::Family::Monospace => font::Family::Monospace,
    }
}

/// Find the caret rectangle for a cursor using layout runs.
///
/// Returns `(x, y, line_height)` in hinted (physical) pixels, where
/// For an empty line, adjust `x_offset` so that a visual indicator of the
/// given `width` stays within the line rather than extending past the right
/// edge (which is what happens with right-aligned text).
fn empty_line_x(
    x_offset: f32,
    width: f32,
    lines: &[cosmic_text::BufferLine],
    line_i: usize,
) -> f32 {
    match lines.get(line_i).and_then(cosmic_text::BufferLine::align) {
        Some(cosmic_text::Align::Right) | Some(cosmic_text::Align::End) => x_offset - width,
        Some(cosmic_text::Align::Center) => x_offset - width / 2.0,
        _ => x_offset,
    }
}

/// `line_height` comes from the matching layout run so it reflects
/// per-line variable heights.
fn caret_position(cursor: cosmic_text::Cursor, buffer: &cosmic_text::Buffer) -> (f32, f32, f32) {
    for run in buffer.layout_runs() {
        if run.line_i != cursor.line {
            continue;
        }

        let start = run.glyphs.first().map(|g| g.start).unwrap_or(0);
        let end = run.glyphs.last().map(|g| g.end).unwrap_or(0);

        // Check if cursor falls on this visual line
        let on_this_line = if start > cursor.index {
            false
        } else {
            match cursor.affinity {
                cosmic_text::Affinity::Before => cursor.index <= end,
                cosmic_text::Affinity::After => cursor.index < end,
            }
        };

        if on_this_line {
            let x = run
                .glyphs
                .iter()
                .take_while(|glyph| cursor.index > glyph.start)
                .last()
                .map(|g| g.x + g.w)
                .unwrap_or_else(|| run.glyphs.first().map(|g| g.x).unwrap_or(run.x_offset));

            return (x, run.line_top, run.line_height);
        }
    }

    // Cursor is past the last run — use the end of the last run on the cursor's line
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    let mut last_h = buffer.metrics().line_height;
    for run in buffer.layout_runs() {
        if run.line_i == cursor.line {
            last_x = run.glyphs.last().map(|g| g.x + g.w).unwrap_or(run.x_offset);
            last_y = run.line_top;
            last_h = run.line_height;
        }
    }

    (last_x, last_y, last_h)
}

fn to_motion(motion: Motion) -> cosmic_text::Motion {
    match motion {
        Motion::Left => cosmic_text::Motion::Left,
        Motion::Right => cosmic_text::Motion::Right,
        Motion::Up => cosmic_text::Motion::Up,
        Motion::Down => cosmic_text::Motion::Down,
        Motion::WordLeft => cosmic_text::Motion::LeftWord,
        Motion::WordRight => cosmic_text::Motion::RightWord,
        Motion::Home => cosmic_text::Motion::Home,
        Motion::End => cosmic_text::Motion::End,
        Motion::PageUp => cosmic_text::Motion::PageUp,
        Motion::PageDown => cosmic_text::Motion::PageDown,
        Motion::DocumentStart => cosmic_text::Motion::BufferStart,
        Motion::DocumentEnd => cosmic_text::Motion::BufferEnd,
    }
}

fn buffer_from_editor<'a, 'b>(editor: &'a impl cosmic_text::Edit<'b>) -> &'a cosmic_text::Buffer
where
    'b: 'a,
{
    match editor.buffer_ref() {
        cosmic_text::BufferRef::Owned(buffer) => buffer,
        cosmic_text::BufferRef::Borrowed(buffer) => buffer,
        cosmic_text::BufferRef::Arc(buffer) => buffer,
    }
}

fn buffer_mut_from_editor<'a, 'b>(
    editor: &'a mut impl cosmic_text::Edit<'b>,
) -> &'a mut cosmic_text::Buffer
where
    'b: 'a,
{
    match editor.buffer_ref_mut() {
        cosmic_text::BufferRef::Owned(buffer) => buffer,
        cosmic_text::BufferRef::Borrowed(buffer) => buffer,
        cosmic_text::BufferRef::Arc(_buffer) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text::rich_editor::Editor as _;

    fn editor(text: &str) -> Editor {
        let mut ed = Editor::with_text(text);
        // Trigger layout so buffer lines are shaped.
        ed.update(
            Size::new(200.0, 200.0),
            Font::default(),
            Pixels(16.0),
            LineHeight::default(),
            Em::ZERO,
            Vec::new(),
            Vec::new(),
            Wrapping::Word,
            None,
            Style::default(),
        );
        ed
    }

    fn line_align(ed: &Editor, line: usize) -> Option<cosmic_text::Align> {
        ed.buffer()
            .lines
            .get(line)
            .and_then(cosmic_text::BufferLine::align)
    }

    #[test]
    fn default_alignment_applies_to_all_lines() {
        let mut ed = editor("line one\nline two\nline three");

        // Initially no explicit alignment.
        assert_eq!(line_align(&ed, 0), None);
        assert_eq!(line_align(&ed, 1), None);
        assert_eq!(line_align(&ed, 2), None);

        ed.align_x(Alignment::Center);

        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Center));
        assert_eq!(line_align(&ed, 1), Some(cosmic_text::Align::Center));
        assert_eq!(line_align(&ed, 2), Some(cosmic_text::Align::Center));
    }

    #[test]
    fn default_alignment_preserves_explicit() {
        let mut ed = editor("line one\nline two\nline three");

        // Explicitly right-align line 1 via paragraph style.
        ed.set_paragraph_style(
            1,
            &rich_editor::paragraph::Style {
                alignment: Some(Alignment::Right),
                ..Default::default()
            },
        );
        assert_eq!(line_align(&ed, 1), Some(cosmic_text::Align::Right));

        // Set default to center — line 1 should stay Right.
        ed.align_x(Alignment::Center);

        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Center));
        assert_eq!(
            line_align(&ed, 1),
            Some(cosmic_text::Align::Right),
            "explicitly set line should keep its alignment"
        );
        assert_eq!(line_align(&ed, 2), Some(cosmic_text::Align::Center));
    }

    #[test]
    fn changing_default_updates_non_explicit_lines() {
        let mut ed = editor("aaa\nbbb");

        ed.align_x(Alignment::Center);
        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Center));
        assert_eq!(line_align(&ed, 1), Some(cosmic_text::Align::Center));

        // Change default to right.
        ed.align_x(Alignment::Right);
        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Right));
        assert_eq!(line_align(&ed, 1), Some(cosmic_text::Align::Right));
    }

    #[test]
    fn setting_default_to_default_restores_none() {
        let mut ed = editor("hello");

        ed.align_x(Alignment::Center);
        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Center));

        // Setting back to Default should restore None.
        ed.align_x(Alignment::Default);
        assert_eq!(line_align(&ed, 0), None);
    }

    #[test]
    fn same_default_is_noop() {
        let mut ed = editor("hello");

        ed.align_x(Alignment::Center);
        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Center));

        // Setting the same value again should not change anything.
        ed.align_x(Alignment::Center);
        assert_eq!(line_align(&ed, 0), Some(cosmic_text::Align::Center));
    }
}
