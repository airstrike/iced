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
use crate::core::{Color, Em, Font, Padding, Pixels, Point, Rectangle, Size};
use crate::text;

use cosmic_text::Edit as _;

use std::borrow::Cow;
use std::collections::HashMap;
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
            text,
            &cosmic_text::Attrs::new(),
            cosmic_text::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(font_system.raw(), false);

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

                let factor = 1.0 / internal.hint_factor;
                let regions = buffer
                    .layout_runs()
                    .flat_map(|run| {
                        let (run_y, run_height) = visible_line_bounds(&run);

                        // Empty lines within the selection range still need a
                        // visible indicator so the highlight looks continuous.
                        if run.glyphs.is_empty()
                            && run.line_i >= start_cursor.line
                            && run.line_i <= end_cursor.line
                        {
                            let w = run.line_height * 0.3;
                            let x = empty_line_x(run.x_offset, w, &buffer.lines, run.line_i);
                            return vec![
                                Rectangle {
                                    x,
                                    width: w,
                                    y: run_y,
                                    height: run_height,
                                } * factor,
                            ];
                        }

                        run.highlight(start_cursor, end_cursor)
                            .filter(|(_, width)| *width > 0.0)
                            .map(|(x, width)| {
                                Rectangle {
                                    x,
                                    width,
                                    y: run_y,
                                    height: run_height,
                                } * factor
                            })
                            .collect()
                    })
                    .collect();

                Selection::Range(regions)
            }
            _ => {
                if let Some((caret_x, caret_y, caret_h)) = caret_position(cursor, buffer) {
                    let f = 1.0 / internal.hint_factor;

                    // Keep the 1px-wide caret within the editor bounds so it
                    // isn't clipped on right-aligned (or end-of-line) text.
                    let x = (caret_x * f).min(internal.bounds.width - 1.0).max(0.0);

                    Selection::Caret(Rectangle::new(
                        Point::new(x, caret_y * f),
                        Size::new(1.0, caret_h * f),
                    ))
                } else {
                    // Cursor's line is scrolled out of view; render
                    // nothing instead of falling back to (0, 0).
                    Selection::Range(Vec::new())
                }
            }
        };

        *internal.selection.write().expect("Write to cursor cache") = Some(cursor.clone());

        cursor
    }

    fn highlight_rect(&self, line: usize, from: usize, to: usize, f: &mut dyn FnMut(Rectangle)) {
        if from >= to {
            return;
        }

        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.document);
        let from_cursor = cosmic_text::Cursor::new(line, from);
        let to_cursor = cosmic_text::Cursor::new(line, to);
        let scale = 1.0 / internal.hint_factor;

        for run in buffer.layout_runs() {
            if run.line_i == line {
                let (y, height) = visible_line_bounds(&run);
                for (x, w) in run.highlight(from_cursor, to_cursor) {
                    if w > 0.0 {
                        f(Rectangle {
                            x: x * scale,
                            width: w * scale,
                            y: y * scale,
                            height: height * scale,
                        });
                    }
                }
            }
        }
    }

    fn decorations(
        &self,
        default_color: Color,
        f: &mut dyn FnMut(rich_editor::Decoration, Rectangle, Color),
    ) {
        // Approach A — render-time decoration resolution.
        //
        // Decorations are no longer baked during shaping; we resolve them here
        // from the live per-line `AttrsList` plus a per-`font_id` cache of
        // font decoration metrics (filled lazily from the global font system
        // on a miss). A decoration/color change is therefore a plain attrs
        // mutation + redraw — no reshape — and this draw-time pass reflects it.
        //
        // We walk each run's glyphs in *visual* order and coalesce consecutive
        // glyphs that resolve to the same `TextDecoration` into a segment. In
        // RTL / bidi runs visual order ≠ byte order, so a single byte span may
        // surface as several visual segments — each gets its own rect, which is
        // exactly what we want.
        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.document);
        let scale = 1.0 / internal.hint_factor;

        let mut metrics_cache: HashMap<
            cosmic_text::fontdb::ID,
            cosmic_text::FontDecorationMetrics,
        > = HashMap::new();

        for run in buffer.layout_runs() {
            let Some(line) = buffer.lines.get(run.line_i) else {
                continue;
            };
            let attrs_list = line.attrs_list();

            let mut segment: Option<DecorationSegment> = None;
            for glyph in run.glyphs {
                let td = attrs_list.text_decoration_at(glyph.start);

                // Flush the open segment when the decoration changes or the run
                // of glyphs sharing it ends.
                let extend = matches!(
                    &segment,
                    Some(seg) if seg.text_decoration == td
                );
                if !extend {
                    if let Some(seg) = segment.take() {
                        seg.emit(&run, scale, default_color, f);
                    }
                    if td.has_decoration() {
                        let metrics = *metrics_cache
                            .entry(glyph.font_id)
                            .or_insert_with(|| font_decoration_metrics(glyph.font_id));
                        segment = Some(DecorationSegment::new(
                            td,
                            metrics,
                            glyph.font_size,
                            glyph.x,
                            glyph.x + glyph.w,
                        ));
                        continue;
                    }
                }

                if let Some(seg) = segment.as_mut() {
                    seg.extend(glyph.x, glyph.x + glyph.w);
                }
            }
            if let Some(seg) = segment.take() {
                seg.emit(&run, scale, default_color, f);
            }
        }
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

                // Mouse events.
                //
                // The click position arrives in *content* space (widget
                // bounds minus padding), but cosmic-text wants buffer
                // space. Two transforms:
                //
                // 1. Subtract `visual_top_pad`: the widget shifted the
                //    buffer origin down by that amount so the first
                //    line's ascender doesn't clip at LH < 1. Hit-test
                //    needs to undo the shift, otherwise a click on the
                //    visible heading lands a line below in buffer space.
                //
                // 2. Clamp clicks in paragraph-spacing gaps onto the
                //    previous slot's bottom. cosmic-text's hit_test
                //    keeps `first_run = true` until it enters a slot;
                //    if the click is past run 0's slot but before
                //    run 1's slot (paragraph spacing gap), iter 2 sees
                //    `first_run && y < line_top` and places the cursor
                //    at the START of run 1 — which is the wrong UX
                //    (Word puts it at the END of the previous line).
                Action::Click(position) => {
                    let raw_x = position.x * internal.hint_factor;
                    let buffer_y = {
                        let buffer = buffer_from_editor(&*editor);
                        let top_pad = text::visual_top_pad(buffer);
                        let raw_y = position.y * internal.hint_factor - top_pad;
                        clamp_click_into_slot(buffer, raw_y)
                    };
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Click {
                            x: raw_x as i32,
                            y: buffer_y as i32,
                        },
                    );
                }
                Action::Drag(position) => {
                    let raw_x = position.x * internal.hint_factor;
                    let buffer_y = {
                        let buffer = buffer_from_editor(&*editor);
                        let top_pad = text::visual_top_pad(buffer);
                        let raw_y = position.y * internal.hint_factor - top_pad;
                        clamp_click_into_slot(buffer, raw_y)
                    };
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Drag {
                            x: raw_x as i32,
                            y: buffer_y as i32,
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

        let buffer = buffer_from_editor(&internal.document);
        let (bounds, _has_rtl) = text::measure(buffer);

        // `measure` excludes the scroll-reserved vertical pad (it cancels in
        // the glyph-overflow math), so add it back so an auto-sized editor
        // allocates room for the top/bottom breathing space.
        let pad = buffer.vertical_pad();
        let mut bounds = bounds * (1.0 / internal.hint_factor);
        bounds.height += (pad.top + pad.bottom) / internal.hint_factor;
        bounds
    }

    fn visual_top_pad(&self) -> f32 {
        let internal = self.internal();
        text::visual_top_pad(buffer_from_editor(&internal.document)) / internal.hint_factor
    }

    fn visual_bottom_pad(&self) -> f32 {
        let internal = self.internal();
        text::visual_bottom_pad(buffer_from_editor(&internal.document)) / internal.hint_factor
    }

    fn hint_factor(&self) -> Option<f32> {
        let internal = self.internal();

        internal.hint.then_some(internal.hint_factor)
    }

    fn update(
        &mut self,
        new_bounds: Size,
        new_padding: Padding,
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

                // Rebuild each line's AttrsList with the new document
                // defaults. Sparse span overrides carry over as-is and
                // re-resolve against the new defaults automatically —
                // this loop used to compare each Attrs field against the
                // old defaults to re-baseline, but cosmic-text's spans
                // are now `AttrsOverride`, so that's free.
                for line in buffer.lines.iter_mut() {
                    let old_spans: Vec<_> = line
                        .attrs_list()
                        .spans()
                        .into_iter()
                        .map(|(range, over)| (range.clone(), over.clone()))
                        .collect();
                    let mut new_list = cosmic_text::AttrsList::new(&default_attrs);
                    for (range, over) in &old_spans {
                        new_list.add_span(range.clone(), over);
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

                buffer.set_hinting(if internal.hint {
                    cosmic_text::Hinting::Enabled
                } else {
                    cosmic_text::Hinting::Disabled
                });

                hinting_changed = true;
            }

            if new_size.0 != metrics.font_size
                || new_line_height.0 != metrics.line_height
                || hinting_changed
            {
                log::trace!("Updating `Metrics` of rich `Editor`...");

                buffer.set_metrics(cosmic_text::Metrics::new(
                    new_size.0 * internal.hint_factor,
                    new_line_height.0 * internal.hint_factor,
                ));
            }

            // Track line_height_ratio for formatting reference
            internal.line_height_ratio = new_line_height.0 / new_size.0;

            let new_wrap = text::to_wrap(new_wrapping);

            if new_wrap != buffer.wrap() {
                log::trace!("Updating `Wrap` strategy of rich `Editor`...");

                buffer.set_wrap(new_wrap);
            }

            // Reserve the vertical padding inside the scroll extent. Applied
            // every update in hinted pixels, so a hint_factor change is picked
            // up automatically. Horizontal padding is already baked into
            // `new_bounds` by the caller.
            buffer.set_vertical_pad(cosmic_text::VerticalPad {
                top: new_padding.top * internal.hint_factor,
                bottom: new_padding.bottom * internal.hint_factor,
            });

            if new_bounds != internal.bounds || hinting_changed {
                log::trace!("Updating size of rich `Editor`...");

                buffer.set_size(
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

                if buffer_line.text().is_empty() {
                    // Empty lines have no characters to span. Apply the
                    // style to the line *defaults* so it takes effect
                    // for future input and is visible in cursor queries.
                    let attrs = style_to_attrs(style, &base, internal.line_height_ratio);
                    let mut new_list = cosmic_text::AttrsList::new(&attrs);
                    // Re-add any existing sparse overrides (shouldn't be
                    // any, but be safe — they'll re-resolve against the
                    // new defaults automatically).
                    for (range, over) in buffer_line.attrs_list().spans() {
                        new_list.add_span(range.clone(), over);
                    }
                    let _ = buffer_line.set_attrs_list(new_list);
                } else {
                    // Build a sparse override carrying only the fields
                    // the caller set. Unspecified fields stay `Inherit`
                    // and resolve to the line defaults at lookup time —
                    // this is what makes paragraph-style changes
                    // re-inherit cleanly (no baked stale values).
                    let over = style_to_override(style, &base, internal.line_height_ratio);
                    let mut new_list = buffer_line.attrs_list().clone();
                    new_list.add_span(range, &over);
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

                // Rebuild AttrsList with new defaults. Sparse overrides
                // carry over as-is — fields they don't set re-resolve
                // against the new defaults at lookup time. No
                // field-by-field re-baselining needed (that workaround
                // existed solely because the old storage baked defaults
                // into each span's `Attrs`).
                let old_spans: Vec<_> = buffer_line
                    .attrs_list()
                    .spans()
                    .into_iter()
                    .map(|(range, over)| (range.clone(), over.clone()))
                    .collect();
                let mut new_list = cosmic_text::AttrsList::new(&defaults);
                for (range, over) in &old_spans {
                    new_list.add_span(range.clone(), over);
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

    fn set_paragraph_spacing(&mut self, line: usize, top: f32, bottom: f32) {
        self.with_internal_mut(|internal| {
            let buffer = buffer_mut_from_editor(&mut internal.document);
            if let Some(buffer_line) = buffer.lines.get_mut(line) {
                let _ = buffer_line.set_margin_top(top);
                let _ = buffer_line.set_margin_bottom(bottom);
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
        let Some(bl) = buffer.lines.get(line) else {
            return Style::default();
        };

        // Empty lines have no characters, so no per-span overrides
        // exist — return an empty (all-`Inherit`) Style. Consumers
        // that want the effective style (e.g. a toolbar showing whether
        // the cursor sits on bold text) merge with paragraph/document
        // defaults at their layer.
        if bl.text().is_empty() {
            return Style::default();
        }

        let idx = if column > 0 && column >= bl.text().len() {
            column - 1
        } else {
            column
        };
        let defaults = bl.attrs_list().defaults();
        let span = bl.attrs_list().get_span(idx);
        attrs_to_sparse_style(&span, &defaults, &internal.font_names)
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
                    style: attrs_to_effective_style(&defaults, &internal.font_names),
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

    fn set_scrollable(&mut self, scrollable: bool) {
        use cosmic_text::Edit as _;
        self.with_internal_mut(|internal| {
            internal.document.set_scrollable(scrollable);
        });
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

/// Build a sparse [`AttrsOverride`] from a [`Style`], composing only the
/// fields the caller explicitly set against `base` for whole-struct
/// overrides (currently `text_decoration`).
///
/// Use this for span overrides — unset fields stay `Inherit` and will
/// resolve against the line defaults at lookup time, which is what
/// prevents the "heading→body keeps size 32" baking bug.
fn style_to_override(
    style: &Style,
    base: &cosmic_text::Attrs<'_>,
    line_height_ratio: f32,
) -> cosmic_text::AttrsOverride {
    use cosmic_text::Override;
    let mut over = cosmic_text::AttrsOverride::default();

    if let Some(bold) = style.bold {
        over.weight = Override::Set(if bold {
            cosmic_text::Weight::BOLD
        } else {
            cosmic_text::Weight::NORMAL
        });
    }

    if let Some(italic) = style.italic {
        over.style = Override::Set(if italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        });
    }

    // text_decoration is a whole-struct override in AttrsOverride. If
    // either underline or strikethrough is set in `style`, compose
    // against `base.text_decoration` so the unspecified field is
    // preserved.
    if style.underline.is_some() || style.strikethrough.is_some() {
        let mut td = base.text_decoration;
        if let Some(underline) = style.underline {
            td.underline = if underline {
                cosmic_text::UnderlineStyle::Single
            } else {
                cosmic_text::UnderlineStyle::None
            };
        }
        if let Some(strikethrough) = style.strikethrough {
            td.strikethrough = strikethrough;
        }
        over.text_decoration = Override::Set(td);
    }

    if let Some(size) = style.size {
        over.metrics = Override::Set(Some(cosmic_text::CacheMetrics::from(
            cosmic_text::Metrics::new(size, size * line_height_ratio),
        )));
    }

    if let Some(color) = style.color {
        over.color = Override::Set(Some(text::to_color(color)));
    }

    if let Some(ls) = style.letter_spacing {
        over.letter_spacing = Override::Set(Some(cosmic_text::LetterSpacing(ls)));
    }

    if let Some(opsz) = style.optical_size {
        over.optical_size = Override::Set(match opsz {
            font::OpticalSize::Auto => cosmic_text::OpticalSize::Auto,
            font::OpticalSize::Fixed(bits) => cosmic_text::OpticalSize::Fixed(f32::from_bits(bits)),
            font::OpticalSize::None => cosmic_text::OpticalSize::None,
        });
    }

    if let Some(font) = style.font {
        // `text::to_attributes` derives family + stretch from the Font.
        // The OLD style_to_attrs effectively ignored the Font's intrinsic
        // weight/style (always overwriting from style.bold/italic or the
        // base attrs); we match that by NOT setting over.weight/over.style
        // from the font.
        let font_attrs = text::to_attributes(font, Em::ZERO, &[], &[]);
        over.family = Override::Set(cosmic_text::FamilyOwned::new(font_attrs.family));
        over.stretch = Override::Set(font_attrs.stretch);
    }

    over
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

/// Build a [`Style`] representing the *effective* character attributes
/// at `attrs` — every applicable field reports its value as `Some(_)`.
///
/// Used by [`paragraph_style_at`], where the returned Style describes
/// the paragraph's own character defaults (e.g. heading-1 is bold +
/// 32px). Consumers compare these against their own theme to decide
/// what's a paragraph-level override.
///
/// Family is reported as `Some(_)` only when it isn't cosmic-text's
/// `SansSerif` sentinel — that variant is the "no font set" default.
fn attrs_to_effective_style(
    attrs: &cosmic_text::Attrs<'_>,
    font_names: &HashSet<&'static str>,
) -> Style {
    Style {
        bold: Some(attrs.weight >= cosmic_text::Weight::BOLD),
        italic: Some(attrs.style == cosmic_text::Style::Italic),
        underline: Some(attrs.text_decoration.underline != cosmic_text::UnderlineStyle::None),
        strikethrough: Some(attrs.text_decoration.strikethrough),
        color: attrs
            .color_opt
            .map(|c| Color::from_rgba8(c.r(), c.g(), c.b(), c.a() as f32 / 255.0)),
        letter_spacing: attrs.letter_spacing_opt.map(|ls| ls.0),
        size: attrs.metrics_opt.map(|m| {
            let m: cosmic_text::Metrics = m.into();
            m.font_size
        }),
        font: match attrs.family {
            cosmic_text::Family::SansSerif => None,
            family => Some(Font {
                family: from_family(family, font_names),
                ..Font::default()
            }),
        },
        optical_size: match attrs.optical_size {
            cosmic_text::OpticalSize::None => None,
            cosmic_text::OpticalSize::Auto => Some(font::OpticalSize::Auto),
            cosmic_text::OpticalSize::Fixed(v) => Some(font::OpticalSize::Fixed(v.to_bits())),
        },
    }
}

/// Build a *sparse* [`Style`] containing only the fields where `attrs`
/// differs from `defaults` — i.e., the per-span override on top of the
/// line defaults. Fields that match `defaults` are returned as `None`,
/// meaning "inherit."
///
/// Used by [`span_style_at`]. Honors the `Option<T>` contract on every
/// Style field: `None` = inherit, `Some(v)` = explicit. Consumers that
/// want the effective value (e.g. a toolbar showing whether the cursor
/// is on bold text) re-merge with paragraph/document defaults at the
/// consumer layer; consumers that want overrides (serializers,
/// op-based history) get them directly.
///
/// Caveat: `Style` cannot represent "explicitly clear" for the
/// already-`Option`-typed fields (color, size, letter_spacing). A span
/// that force-clears a non-None default is reported as `None` (inherit)
/// for now. Round-tripping force-clears would need a richer Style type.
fn attrs_to_sparse_style(
    attrs: &cosmic_text::Attrs<'_>,
    defaults: &cosmic_text::Attrs<'_>,
    font_names: &HashSet<&'static str>,
) -> Style {
    let bold = attrs.weight >= cosmic_text::Weight::BOLD;
    let italic = attrs.style == cosmic_text::Style::Italic;
    let underline = attrs.text_decoration.underline != cosmic_text::UnderlineStyle::None;
    let strikethrough = attrs.text_decoration.strikethrough;

    let bold_default = defaults.weight >= cosmic_text::Weight::BOLD;
    let italic_default = defaults.style == cosmic_text::Style::Italic;
    let underline_default = defaults.text_decoration.underline != cosmic_text::UnderlineStyle::None;
    let strikethrough_default = defaults.text_decoration.strikethrough;

    Style {
        bold: (bold != bold_default).then_some(bold),
        italic: (italic != italic_default).then_some(italic),
        underline: (underline != underline_default).then_some(underline),
        strikethrough: (strikethrough != strikethrough_default).then_some(strikethrough),
        color: if attrs.color_opt != defaults.color_opt {
            attrs
                .color_opt
                .map(|c| Color::from_rgba8(c.r(), c.g(), c.b(), c.a() as f32 / 255.0))
        } else {
            None
        },
        size: if attrs.metrics_opt != defaults.metrics_opt {
            attrs.metrics_opt.map(|m| {
                let m: cosmic_text::Metrics = m.into();
                m.font_size
            })
        } else {
            None
        },
        letter_spacing: if attrs.letter_spacing_opt != defaults.letter_spacing_opt {
            attrs.letter_spacing_opt.map(|ls| ls.0)
        } else {
            None
        },
        font: if attrs.family != defaults.family {
            Some(Font {
                family: from_family(attrs.family, font_names),
                ..Font::default()
            })
        } else {
            None
        },
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
/// Compute the caret's `(x, y, height)` for `cursor`, or `None`
/// when the cursor's line has no laid-out run in the buffer's current
/// visible range (i.e. scrolled out of view). The caller suppresses
/// caret rendering in the `None` case.
///
/// `y` and `height` use [`visible_line_bounds`] so the caret reaches
/// the topmost ascender and bottommost descender of the line, not
/// just the layout slot — important when the line slot is compact
/// (e.g. user-chosen `line_height` smaller than the font's natural
/// extent) where the slot is significantly shorter than the inked
/// glyphs.
fn caret_position(
    cursor: cosmic_text::Cursor,
    buffer: &cosmic_text::Buffer,
) -> Option<(f32, f32, f32)> {
    let mut last_on_line: Option<(f32, f32, f32)> = None;

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

        let (y, height) = visible_line_bounds(&run);

        if on_this_line {
            let x = run
                .glyphs
                .iter()
                .take_while(|glyph| cursor.index > glyph.start)
                .last()
                .map(|g| g.x + g.w)
                .unwrap_or_else(|| run.glyphs.first().map(|g| g.x).unwrap_or(run.x_offset));

            return Some((x, y, height));
        }

        // Cursor is past the end of this run — remember it as the
        // past-the-end candidate for the cursor's line.
        let x = run.glyphs.last().map(|g| g.x + g.w).unwrap_or(run.x_offset);
        last_on_line = Some((x, y, height));
    }

    // If we saw any run on the cursor's line but the cursor was past
    // the end, fall back to the last seen position. Otherwise the
    // cursor's line is not in the visible layout (scrolled away) and
    // there is no caret to draw.
    last_on_line
}

/// Returns the line's visible vertical bounds: `(y, height)`. Covers
/// both the layout slot (`line_top..line_top + line_height`) and the
/// inked glyph extent (`line_y - max_ascent..line_y + max_descent`),
/// taking the union so:
///
/// - At loose line-heights (slot ⊇ glyphs) the slot is returned —
///   selection/caret matches the user's chosen line spacing including
///   whitespace above/below glyphs.
/// - At compact line-heights (glyphs overflow the slot, e.g. user
///   picked LH < 1.0, or a font like Instrument Serif whose descender
///   exceeds the typical metric), the inked extent is returned so the
///   selection rect and caret reach all the visible pixels.
fn visible_line_bounds(run: &cosmic_text::LayoutRun<'_>) -> (f32, f32) {
    let glyph_top = run.line_y - run.max_ascent;
    let glyph_bottom = run.line_y + run.max_descent;
    let slot_top = run.line_top;
    let slot_bottom = run.line_top + run.line_height;
    let top = glyph_top.min(slot_top);
    let bottom = glyph_bottom.max(slot_bottom);
    (top, bottom - top)
}

fn from_cosmic_color(color: cosmic_text::Color) -> Color {
    Color::from_rgba8(color.r(), color.g(), color.b(), color.a() as f32 / 255.0)
}

/// Look up a font's decoration metrics through the global font system,
/// loading the face on a miss. The caller caches the result per `font_id`,
/// so this runs at most once per font per draw.
fn font_decoration_metrics(font_id: cosmic_text::fontdb::ID) -> cosmic_text::FontDecorationMetrics {
    text::font_system()
        .write()
        .ok()
        .and_then(|mut fs| fs.raw().decoration_metrics(font_id))
        .unwrap_or_default()
}

/// A run of visually-contiguous glyphs sharing one resolved
/// [`cosmic_text::TextDecoration`], accumulated while walking a layout run.
///
/// Holds the x-extent (`x_min..x_max`) and the per-font metrics + font size
/// needed to place the decoration lines at draw. [`Self::emit`] turns it into
/// the underline / strikethrough / overline rects.
struct DecorationSegment {
    text_decoration: cosmic_text::TextDecoration,
    metrics: cosmic_text::FontDecorationMetrics,
    font_size: f32,
    x_min: f32,
    x_max: f32,
}

impl DecorationSegment {
    fn new(
        text_decoration: cosmic_text::TextDecoration,
        metrics: cosmic_text::FontDecorationMetrics,
        font_size: f32,
        x_start: f32,
        x_end: f32,
    ) -> Self {
        Self {
            text_decoration,
            metrics,
            font_size,
            x_min: x_start,
            x_max: x_end,
        }
    }

    fn extend(&mut self, x_start: f32, x_end: f32) {
        self.x_min = self.x_min.min(x_start);
        self.x_max = self.x_max.max(x_end);
    }

    fn emit(
        &self,
        run: &cosmic_text::LayoutRun<'_>,
        scale: f32,
        default_color: Color,
        f: &mut dyn FnMut(rich_editor::Decoration, Rectangle, Color),
    ) {
        let width = self.x_max - self.x_min;
        if width <= 0.0 {
            return;
        }
        let font_size = self.font_size;
        let td = &self.text_decoration;

        let resolve = |override_opt: Option<cosmic_text::Color>| -> Color {
            override_opt.map(from_cosmic_color).unwrap_or(default_color)
        };

        match td.underline {
            cosmic_text::UnderlineStyle::None => {}
            cosmic_text::UnderlineStyle::Single | cosmic_text::UnderlineStyle::Double => {
                let color = resolve(td.underline_color_opt);
                let thickness = (self.metrics.underline.thickness * font_size)
                    .max(1.0)
                    .ceil();
                let y = run.line_y - self.metrics.underline.offset * font_size;
                let rect = Rectangle {
                    x: self.x_min,
                    y,
                    width,
                    height: thickness,
                };
                match td.underline {
                    cosmic_text::UnderlineStyle::Single => {
                        f(rich_editor::Decoration::Underline, rect * scale, color);
                    }
                    cosmic_text::UnderlineStyle::Double => {
                        f(
                            rich_editor::Decoration::DoubleUnderline,
                            rect * scale,
                            color,
                        );
                        let second = Rectangle {
                            x: self.x_min,
                            y: y + thickness * 2.0,
                            width,
                            height: thickness,
                        };
                        f(
                            rich_editor::Decoration::DoubleUnderline,
                            second * scale,
                            color,
                        );
                    }
                    cosmic_text::UnderlineStyle::None => {}
                }
            }
        }

        if td.strikethrough {
            let color = resolve(td.strikethrough_color_opt);
            let thickness = (self.metrics.strikethrough.thickness * font_size)
                .max(1.0)
                .ceil();
            let y = run.line_y - self.metrics.strikethrough.offset * font_size;
            f(
                rich_editor::Decoration::Strikethrough,
                Rectangle {
                    x: self.x_min,
                    y,
                    width,
                    height: thickness,
                } * scale,
                color,
            );
        }

        if td.overline {
            let color = resolve(td.overline_color_opt);
            let thickness = (self.metrics.underline.thickness * font_size)
                .max(1.0)
                .ceil();
            let y = (run.line_y - self.metrics.ascent * font_size).max(run.line_top);
            f(
                rich_editor::Decoration::Overline,
                Rectangle {
                    x: self.x_min,
                    y,
                    width,
                    height: thickness,
                } * scale,
                color,
            );
        }
    }
}

/// Clamp `y` so it lands inside *some* slot rather than in a
/// paragraph-spacing gap.
///
/// cosmic-text's hit_test only places a cursor when `y` falls inside
/// a layout slot (`[line_top, line_top + line_height)`); if `y` is in
/// a gap between two slots (because the lower paragraph carries a
/// `space_before` or the upper paragraph carries a `spacing_after`),
/// the iterator's `first_run` flag stays true until the next slot
/// engages, and cosmic-text ends up placing the cursor at the START
/// of that next slot. Word's behavior is to place it at the END of
/// the previous line. We replicate that by pulling `y` just inside
/// the previous slot before forwarding to cosmic-text.
///
/// Pre-first-slot and past-last-slot clicks are handed through
/// unchanged — cosmic-text already handles those (cursor lands at
/// start of first line / end of last line respectively).
fn clamp_click_into_slot(buffer: &cosmic_text::Buffer, y: f32) -> f32 {
    let mut prev_slot_bottom: Option<f32> = None;
    for run in buffer.layout_runs() {
        let slot_top = run.line_top;
        let slot_bottom = slot_top + run.line_height;
        if y < slot_top {
            if let Some(prev_bottom) = prev_slot_bottom {
                // Click is in the gap between previous and this run.
                // Pull just inside the previous slot.
                return (prev_bottom - 1.0).max(0.0);
            }
            // No previous run — click is above the very first slot.
            return y;
        }
        if y < slot_bottom {
            // Inside a slot — no adjustment needed.
            return y;
        }
        prev_slot_bottom = Some(slot_bottom);
    }
    // Past every slot — leave for cosmic-text to clamp to last line end.
    y
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
        editor_with(text, Pixels(16.0), LineHeight::default())
    }

    fn editor_with(text: &str, size: Pixels, line_height: LineHeight) -> Editor {
        let mut ed = Editor::with_text(text);
        // Trigger layout so buffer lines are shaped.
        ed.update(
            Size::new(200.0, 200.0),
            Padding::ZERO,
            Font::default(),
            size,
            line_height,
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

    /// `span_style_at` returns a *sparse* Style — None for fields that
    /// match the current line defaults, Some only for explicit overrides.
    /// Under heading-1 defaults, a span that only sets italic reports
    /// italic=Some(true) and everything else None.
    #[test]
    fn span_style_at_reports_sparse_override() {
        let mut ed = editor("AGENTS.md");

        ed.set_paragraph_style(
            0,
            &rich_editor::paragraph::Style {
                style: Style {
                    bold: Some(true),
                    size: Some(32.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        ed.set_span_style(
            0,
            0..9,
            &Style {
                italic: Some(true),
                ..Default::default()
            },
        );

        let s = ed.span_style_at(0, 3);
        assert_eq!(s.italic, Some(true), "explicit italic shows as override");
        assert_eq!(s.bold, None, "bold matches defaults — not in sparse Style");
        assert_eq!(s.size, None, "size matches defaults — not in sparse Style");
    }

    /// Bug B regression: when paragraph defaults change (heading→body),
    /// the span's *unspecified* fields re-resolve against the new
    /// defaults. The sparse Style still reports only what the span
    /// explicitly set — but the in-cosmic-text effective resolution
    /// follows the new defaults.
    #[test]
    fn heading_to_body_demotion_inherits_new_defaults() {
        let mut ed = editor("AGENTS.md");

        ed.set_paragraph_style(
            0,
            &rich_editor::paragraph::Style {
                style: Style {
                    bold: Some(true),
                    size: Some(32.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        ed.set_span_style(
            0,
            0..9,
            &Style {
                italic: Some(true),
                ..Default::default()
            },
        );

        // While heading defaults are in effect, sparse Style only has italic.
        let s = ed.span_style_at(0, 3);
        assert_eq!(s.italic, Some(true));
        assert_eq!(s.bold, None);
        assert_eq!(s.size, None);

        // Demote to body.
        ed.set_paragraph_style(0, &rich_editor::paragraph::Style::default());

        // The override is unchanged (still just italic). Bold and size
        // are still `Inherit` — and against the new body defaults
        // (no bold, no size), they still report None. This is the
        // structural guarantee: sparse spans + sparse reads = no
        // baking, no stale heading attrs leaking through.
        let s = ed.span_style_at(0, 3);
        assert_eq!(s.italic, Some(true), "explicit italic survives demotion");
        assert_eq!(s.bold, None);
        assert_eq!(s.size, None);
    }

    /// Counterpart: a span that *explicitly* sets bold matches heading
    /// defaults (so sparse Style reports None while in heading-1 — the
    /// override matches defaults, indistinguishable from inherit in the
    /// sparse view). After demoting to body, defaults' bold goes away
    /// and the explicit override reveals itself as Some(true) in the
    /// sparse Style.
    #[test]
    fn explicit_span_fields_revealed_after_demotion() {
        let mut ed = editor("AGENTS.md");

        ed.set_paragraph_style(
            0,
            &rich_editor::paragraph::Style {
                style: Style {
                    bold: Some(true),
                    size: Some(32.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        // Span explicitly sets bold=true — but this matches heading-1
        // defaults so the sparse view reports None.
        ed.set_span_style(
            0,
            0..9,
            &Style {
                bold: Some(true),
                ..Default::default()
            },
        );

        let s = ed.span_style_at(0, 3);
        assert_eq!(
            s.bold, None,
            "while bold matches defaults, sparse view reports None"
        );

        // Demote: now defaults' bold is gone, so the explicit
        // override is visible in the sparse Style.
        ed.set_paragraph_style(0, &rich_editor::paragraph::Style::default());

        let s = ed.span_style_at(0, 3);
        assert_eq!(
            s.bold,
            Some(true),
            "explicit bold survives in cosmic-text storage and now \
             differs from body defaults — sparse view exposes it"
        );
        assert_eq!(s.size, None, "size was inherit-only — drops to None");
    }

    /// Verifies `paragraph_style_at` reports the line's current
    /// defaults, independent of any sparse span overrides on that
    /// line. Reading the paragraph style after a demotion should
    /// reflect the new (body) defaults, not the old (heading) ones.
    #[test]
    fn paragraph_style_at_tracks_current_defaults() {
        let mut ed = editor("AGENTS.md");

        ed.set_paragraph_style(
            0,
            &rich_editor::paragraph::Style {
                style: Style {
                    bold: Some(true),
                    size: Some(32.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        // Add a span override that DIFFERS from defaults — to confirm
        // the paragraph-style read doesn't accidentally see span attrs.
        ed.set_span_style(
            0,
            0..9,
            &Style {
                color: Some(Color::from_rgb8(255, 0, 0)),
                ..Default::default()
            },
        );

        let para = ed.paragraph_style_at(0);
        assert_eq!(para.style.bold, Some(true));
        assert_eq!(para.style.size, Some(32.0));

        // Demote.
        ed.set_paragraph_style(0, &rich_editor::paragraph::Style::default());

        let para = ed.paragraph_style_at(0);
        assert_eq!(para.style.bold, Some(false));
        assert_eq!(para.style.size, None);
    }

    // ── Glyph-overflow / visual_top_pad / measure ─────────────────────────────

    /// Default line-height (Relative(1.3)) gives the font more vertical
    /// room than its natural ascent+descent, so cosmic-text's centering
    /// distributes excess space above+below the glyph and there's no
    /// overflow. `visual_top_pad` is 0, `min_bounds().height` equals the
    /// sum of layout-run heights.
    #[test]
    fn loose_line_height_has_no_visual_overflow() {
        let ed = editor_with("hello", Pixels(16.0), LineHeight::Relative(1.3));
        assert_eq!(
            ed.visual_top_pad(),
            0.0,
            "loose LH should have no top overflow",
        );
        // min_bounds().height should equal the single line's height ≈
        // 16 × 1.3 = 20.8. Allow a small slack for hinting / rounding.
        let h = ed.min_bounds().height;
        assert!(
            (h - 20.8).abs() < 1.0,
            "expected min_bounds.height ≈ 20.8 for one line at 16px @ LH 1.3, got {h}",
        );
    }

    /// When `line_height < max_ascent + max_descent`, cosmic-text
    /// centers glyphs around the baseline — the overflow shows up
    /// symmetrically above `line_top` and below `line_top + line_height`.
    /// `visual_top_pad` reports the top half; `min_bounds.height` adds
    /// both halves to the layout-summed height.
    #[test]
    fn compact_line_height_reports_glyph_overflow() {
        // 28px text at LH 0.5 → 14px slot. The font's natural extent
        // is ~1.15-1.2 × font_size ≈ 32-34px. So we expect ~10px of
        // overflow distributed symmetrically: ~5px above, ~5px below.
        let ed = editor_with("hello", Pixels(28.0), LineHeight::Relative(0.5));

        let top_pad = ed.visual_top_pad();
        assert!(
            top_pad > 4.0 && top_pad < 12.0,
            "expected visual_top_pad in (4, 12) for 28px @ LH 0.5, got {top_pad}",
        );

        // Slot = 14px, but min_bounds should include the overflow:
        // ≈ 14 + 2 × top_pad. Symmetric centering means top_pad ≈
        // bottom_overflow, so min_bounds.height ≈ slot + 2 × top_pad.
        let h = ed.min_bounds().height;
        let expected = 14.0 + 2.0 * top_pad;
        assert!(
            (h - expected).abs() < 1.0,
            "min_bounds.height = {h}, expected ≈ slot(14) + 2 × top_pad({top_pad}) = {expected}",
        );
    }

    /// Two-line buffer with compact LH: the top overflow is only on
    /// the first line, the bottom overflow only on the last. Inner
    /// runs' overflow is absorbed by neighbors — `min_bounds` doesn't
    /// double-count.
    #[test]
    fn multi_line_overflow_counts_only_first_and_last() {
        let ed = editor_with("hello\nworld", Pixels(28.0), LineHeight::Relative(0.5));
        let top_pad = ed.visual_top_pad();
        let h = ed.min_bounds().height;

        // Two slots of 14px each = 28px layout height.
        // Plus first-line top overflow + last-line bottom overflow,
        // each ≈ top_pad (symmetric centering).
        let expected = 28.0 + 2.0 * top_pad;
        assert!(
            (h - expected).abs() < 1.0,
            "two-line min_bounds.height = {h}, expected ≈ 2 × slot(14) + 2 × top_pad({top_pad}) = {expected}",
        );
    }

    /// An empty buffer shouldn't panic and should report 0 top_pad
    /// (no glyphs, nothing to overflow).
    #[test]
    fn empty_buffer_has_zero_top_pad() {
        let ed = editor_with("", Pixels(16.0), LineHeight::Relative(0.5));
        assert_eq!(ed.visual_top_pad(), 0.0);
    }

    // ── Selection / caret cover the glyph extent at compact LH ────────────────

    /// Helper: read the first layout run's glyph extent (max_ascent +
    /// max_descent) so tests can compare selection geometry against
    /// the actual inked area, not the layout slot.
    fn first_run_glyph_extent(ed: &Editor) -> f32 {
        let internal = ed.internal();
        let buffer = buffer_from_editor(&internal.document);
        let first = buffer
            .layout_runs()
            .next()
            .expect("buffer should have at least one layout run");
        (first.max_ascent + first.max_descent) / internal.hint_factor
    }

    /// Helper: build a Cursor selecting columns 0..end on line 0.
    fn select_first_chars(end: usize) -> editor::Cursor {
        editor::Cursor {
            position: editor::Position {
                line: 0,
                column: end,
            },
            selection: Some(editor::Position { line: 0, column: 0 }),
        }
    }

    /// Bug repro: at compact line-height the selection rectangle is
    /// drawn at the line slot's size, which is shorter than the
    /// actual visible glyphs. The rect should at minimum cover the
    /// glyph extent (`max_ascent + max_descent`) — selecting a
    /// character should highlight the WHOLE glyph, including
    /// ascenders and descenders that overflow the slot.
    #[test]
    fn selection_rect_covers_glyph_extent_at_compact_line_height() {
        let mut ed = editor_with("hello", Pixels(28.0), LineHeight::Relative(0.5));
        let glyph_extent = first_run_glyph_extent(&ed);
        // Sanity: glyph extent should be substantially larger than
        // the 14px slot for 28px @ LH 0.5.
        assert!(
            glyph_extent > 25.0,
            "glyph extent should be ~32 for 28px font, got {glyph_extent}",
        );

        ed.move_to(select_first_chars(3));

        let selection = ed.selection();
        let rects = match selection {
            editor::Selection::Range(rects) => rects,
            other @ editor::Selection::Caret(_) => {
                panic!("expected Selection::Range, got {other:?}")
            }
        };
        assert!(!rects.is_empty(), "expected at least one selection rect");

        for rect in &rects {
            assert!(
                rect.height >= glyph_extent - 0.5,
                "selection rect height {} should cover full glyph extent {glyph_extent}; \
                 line slot at LH 0.5 is only 14px, so a value near 14 means the rect \
                 was drawn at the slot's height and is clipping ascenders/descenders",
                rect.height,
            );
        }
    }

    /// Bug repro (caret): same problem as the selection range — the
    /// caret rectangle is drawn at `line_height`, so at compact LH
    /// the blinking caret is a short stub that doesn't cover the
    /// glyph extent. It should reach from the topmost ascender to
    /// the bottommost descender.
    #[test]
    fn caret_height_covers_glyph_extent_at_compact_line_height() {
        let mut ed = editor_with("hello", Pixels(28.0), LineHeight::Relative(0.5));
        let glyph_extent = first_run_glyph_extent(&ed);

        // Cursor with no selection — should produce a Caret variant.
        ed.move_to(editor::Cursor {
            position: editor::Position { line: 0, column: 2 },
            selection: None,
        });

        let caret = match ed.selection() {
            editor::Selection::Caret(rect) => rect,
            other @ editor::Selection::Range(_) => {
                panic!("expected Selection::Caret, got {other:?}")
            }
        };

        assert!(
            caret.height >= glyph_extent - 0.5,
            "caret height {} should cover full glyph extent {glyph_extent}; \
             current code uses line_height (14px @ LH 0.5), producing a stub caret \
             that doesn't reach the ascenders or descenders",
            caret.height,
        );
    }

    /// Sanity: at loose LH (where the slot is *larger* than the
    /// glyph extent), the selection rect should remain the slot
    /// size — covering the whitespace above/below glyphs is the
    /// expected behavior for line-spaced text. The fix should be
    /// "at least the glyph extent," not "exactly the glyph extent."
    #[test]
    fn selection_rect_matches_slot_at_loose_line_height() {
        let mut ed = editor_with("hello", Pixels(16.0), LineHeight::Relative(1.5));
        ed.move_to(select_first_chars(3));

        let rects = match ed.selection() {
            editor::Selection::Range(rects) => rects,
            other @ editor::Selection::Caret(_) => {
                panic!("expected Selection::Range, got {other:?}")
            }
        };

        // 16 * 1.5 = 24 line slot, glyph extent ≈ 16 * 1.15 = 18.4.
        // Rect should be ~24 (the slot), not 18.
        for rect in &rects {
            assert!(
                (rect.height - 24.0).abs() < 1.0,
                "loose LH: rect should match line slot (24px), got {}",
                rect.height,
            );
        }
    }

    // ── Decoration rendering (underline / strikethrough) ─────────────────────

    /// Helper: collect decoration callback emissions into a Vec.
    fn collect_decorations(ed: &Editor) -> Vec<(rich_editor::Decoration, Rectangle, Color)> {
        let mut out = Vec::new();
        ed.decorations(Color::BLACK, &mut |kind, rect, color| {
            out.push((kind, rect, color));
        });
        out
    }

    /// `set_span_style` modifies the AttrsList and marks the layout
    /// dirty; `buffer.layout_runs()` won't reshape on demand, so the
    /// next thing that needs glyphs must trigger shape. We just call
    /// `update` again — it ends in `shape_until_scroll`.
    fn reshape(ed: &mut Editor) {
        ed.update(
            Size::new(200.0, 200.0),
            Padding::ZERO,
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
    }

    /// Bug repro: a span with `underline: Some(true)` should produce
    /// exactly one Underline decoration rect over its range. With no
    /// implementation, this returns nothing — the markdown sample's
    /// "and underline styles" doesn't render an underline.
    #[test]
    fn underline_span_emits_one_underline_decoration() {
        let mut ed = editor("hello world");
        ed.set_span_style(
            0,
            0..5,
            &Style {
                underline: Some(true),
                ..Default::default()
            },
        );

        reshape(&mut ed);
        let decos = collect_decorations(&ed);
        let underlines: Vec<_> = decos
            .iter()
            .filter(|(k, _, _)| *k == rich_editor::Decoration::Underline)
            .collect();

        assert_eq!(
            underlines.len(),
            1,
            "expected exactly one underline decoration for a single-line span, got {decos:?}",
        );
        let (_, rect, _) = underlines[0];
        assert!(
            rect.width > 0.0 && rect.height > 0.0,
            "underline rect should have positive dimensions, got {rect:?}",
        );
        // Underline sits just below the baseline — for a 16px font
        // with default LH, baseline lands ~16-18px from the slot top,
        // so the underline rect's y should be below ~12.
        assert!(
            rect.y > 12.0,
            "underline should sit below the baseline, got y = {}",
            rect.y,
        );
    }

    /// A span with `strikethrough: Some(true)` should produce a
    /// Strikethrough rect, NOT an underline. They use different font
    /// metrics (strikethrough_metrics vs underline_metrics) and are
    /// positioned through the x-height region.
    #[test]
    fn strikethrough_span_emits_strikethrough_decoration() {
        let mut ed = editor("hello world");
        ed.set_span_style(
            0,
            0..5,
            &Style {
                strikethrough: Some(true),
                ..Default::default()
            },
        );

        reshape(&mut ed);
        let decos = collect_decorations(&ed);
        let kinds: Vec<rich_editor::Decoration> = decos.iter().map(|(k, _, _)| *k).collect();

        assert!(
            kinds.contains(&rich_editor::Decoration::Strikethrough),
            "expected a Strikethrough decoration, got kinds: {kinds:?}",
        );
        assert!(
            !kinds.contains(&rich_editor::Decoration::Underline),
            "strikethrough-only span should not emit an underline, got kinds: {kinds:?}",
        );
    }

    /// A span without any decoration set should emit no decoration
    /// callbacks. Drives the "plain bold span isn't accidentally
    /// underlined" guarantee.
    #[test]
    fn plain_span_emits_no_decoration() {
        let mut ed = editor("hello world");
        ed.set_span_style(
            0,
            0..5,
            &Style {
                bold: Some(true),
                ..Default::default()
            },
        );

        reshape(&mut ed);
        let decos = collect_decorations(&ed);
        assert!(
            decos.is_empty(),
            "bold-only span should emit no decorations, got {decos:?}",
        );
    }
}
