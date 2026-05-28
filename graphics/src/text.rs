//! Draw text.
pub mod cache;
pub mod editor;
pub mod paragraph;

pub use cache::Cache;
pub use editor::Editor;
pub use paragraph::Paragraph;

pub use cosmic_text;

use crate::core::alignment;
use crate::core::font::{self, Font};
use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use crate::core::{Color, Em, Pixels, Point, Rectangle, Size, Transformation};
use crate::rich;

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock, RwLock, Weak};

/// A text primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    /// A paragraph.
    #[allow(missing_docs)]
    Paragraph {
        paragraph: paragraph::Weak,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    },
    /// An editor.
    #[allow(missing_docs)]
    Editor {
        editor: editor::Weak,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    },
    /// A rich editor.
    #[allow(missing_docs)]
    RichEditor {
        editor: rich::editor::Weak,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    },
    /// Some cached text.
    Cached {
        /// The contents of the text.
        content: String,
        /// The bounds of the text.
        bounds: Rectangle,
        /// The color of the text.
        color: Color,
        /// The size of the text in logical pixels.
        size: Pixels,
        /// The line height of the text.
        line_height: Pixels,
        /// The font of the text.
        font: Font,
        /// The horizontal alignment of the text.
        align_x: Alignment,
        /// The vertical alignment of the text.
        align_y: alignment::Vertical,
        /// The shaping strategy of the text.
        shaping: Shaping,
        /// The wrapping strategy of the text.
        wrapping: Wrapping,
        /// The ellipsis strategy of the text.
        ellipsis: Ellipsis,
        /// The letter spacing of the text.
        letter_spacing: Em,
        /// The font features of the text.
        font_features: Vec<font::Feature>,
        /// The font variations of the text.
        font_variations: Vec<font::Variation>,
        /// The clip bounds of the text.
        clip_bounds: Rectangle,
    },
    /// Some raw text.
    #[allow(missing_docs)]
    Raw {
        raw: Raw,
        transformation: Transformation,
    },
}

impl Text {
    /// Returns the visible bounds of the [`Text`].
    pub fn visible_bounds(&self) -> Option<Rectangle> {
        match self {
            Text::Paragraph {
                position,
                paragraph,
                clip_bounds,
                transformation,
                ..
            } => Rectangle::new(*position, paragraph.min_bounds)
                .intersection(clip_bounds)
                .map(|bounds| bounds * *transformation),
            Text::Editor {
                editor,
                position,
                clip_bounds,
                transformation,
                ..
            } => Rectangle::new(*position, editor.bounds)
                .intersection(clip_bounds)
                .map(|bounds| bounds * *transformation),
            Text::RichEditor {
                editor,
                position,
                clip_bounds,
                transformation,
                ..
            } => Rectangle::new(*position, editor.bounds)
                .intersection(clip_bounds)
                .map(|bounds| bounds * *transformation),
            Text::Cached {
                bounds,
                clip_bounds,
                ..
            } => bounds.intersection(clip_bounds),
            Text::Raw { raw, .. } => Some(raw.clip_bounds),
        }
    }
}

/// The regular variant of the [Fira Sans] font.
///
/// It is loaded as part of the default fonts when the `fira-sans`
/// feature is enabled.
///
/// [Fira Sans]: https://mozilla.github.io/Fira/
#[cfg(feature = "fira-sans")]
pub const FIRA_SANS_REGULAR: &[u8] = include_bytes!("../fonts/FiraSans-Regular.ttf").as_slice();

/// Returns the global [`FontSystem`].
pub fn font_system() -> &'static RwLock<FontSystem> {
    static FONT_SYSTEM: OnceLock<RwLock<FontSystem>> = OnceLock::new();

    FONT_SYSTEM.get_or_init(|| {
        #[allow(unused_mut)]
        let mut raw = cosmic_text::FontSystem::new_with_fonts([
            cosmic_text::fontdb::Source::Binary(Arc::new(
                include_bytes!("../fonts/Iced-Icons.ttf").as_slice(),
            )),
            #[cfg(feature = "fira-sans")]
            cosmic_text::fontdb::Source::Binary(Arc::new(
                include_bytes!("../fonts/FiraSans-Regular.ttf").as_slice(),
            )),
        ]);

        #[cfg(feature = "fira-sans")]
        raw.db_mut().set_sans_serif_family("Fira Sans");

        #[cfg(target_os = "macos")]
        {
            #[cfg(not(feature = "fira-sans"))]
            raw.db_mut().set_sans_serif_family(".SF NS");
            raw.db_mut().set_serif_family("Times New Roman");
            raw.db_mut().set_monospace_family("Menlo");
        }

        #[cfg(target_os = "windows")]
        {
            #[cfg(not(feature = "fira-sans"))]
            raw.db_mut().set_sans_serif_family("Segoe UI");
            raw.db_mut().set_serif_family("Times New Roman");
            raw.db_mut().set_monospace_family("Consolas");
        }

        RwLock::new(FontSystem {
            raw,
            loaded_fonts: HashSet::new(),
            version: Version::default(),
        })
    })
}

/// A set of system fonts.
pub struct FontSystem {
    raw: cosmic_text::FontSystem,
    loaded_fonts: HashSet<usize>,
    version: Version,
}

impl FontSystem {
    /// Returns the raw [`cosmic_text::FontSystem`].
    pub fn raw(&mut self) -> &mut cosmic_text::FontSystem {
        &mut self.raw
    }

    /// Loads a font from its bytes.
    pub fn load_font(&mut self, bytes: Cow<'static, [u8]>) {
        if let Cow::Borrowed(bytes) = bytes {
            let address = bytes.as_ptr() as usize;

            if !self.loaded_fonts.insert(address) {
                return;
            }
        }

        let _ = self
            .raw
            .db_mut()
            .load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
                bytes.into_owned(),
            )));

        self.version = Version(self.version.0 + 1);
    }

    /// Returns an iterator over the family names of all font faces
    /// in the font database.
    pub fn families(&self) -> impl Iterator<Item = &str> {
        self.raw
            .db()
            .faces()
            .filter_map(|face| face.families.first())
            .map(|(name, _)| name.as_str())
    }

    /// Returns the current [`Version`] of the [`FontSystem`].
    ///
    /// Loading a font will increase the version of a [`FontSystem`].
    pub fn version(&self) -> Version {
        self.version
    }
}

/// A version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version(u32);

/// A weak reference to a [`cosmic_text::Buffer`] that can be drawn.
#[derive(Debug, Clone)]
pub struct Raw {
    /// A weak reference to a [`cosmic_text::Buffer`].
    pub buffer: Weak<cosmic_text::Buffer>,
    /// The position of the text.
    pub position: Point,
    /// The color of the text.
    pub color: Color,
    /// The clip bounds of the text.
    pub clip_bounds: Rectangle,
}

impl PartialEq for Raw {
    fn eq(&self, _other: &Self) -> bool {
        // TODO: There is no proper way to compare raw buffers
        // For now, no two instances of `Raw` text will be equal.
        // This should be fine, but could trigger unnecessary redraws
        // in the future.
        false
    }
}

/// Measures the *visible* dimensions of the given [`cosmic_text::Buffer`].
///
/// Three things go into the height:
///
/// 1. Sum of layout-run `line_height`s (one per wrapped line).
/// 2. Each laid-out buffer line's `margin_top` + `margin_bottom`,
///    exactly once.
/// 3. Glyph overflow on the first and last visible runs. When a
///    user-chosen `line_height` is shorter than the font's natural
///    `max_ascent + max_descent`, cosmic-text centers glyphs around
///    the baseline — which means the first line's ascenders extend
///    *above* `line_top = 0` and the last line's descenders extend
///    *below* `line_top + line_height`. Without accounting for that
///    overflow, the widget allocates too little vertical space and
///    glyphs at the document boundaries clip against the editor's
///    scissor.
///
/// The returned height reflects the inked region the renderer needs
/// to draw without clipping. Pair with [`visual_top_pad`] to learn
/// how far the buffer's `(0, 0)` should be shifted from the widget's
/// content-area origin so the first line's ascenders land inside the
/// allocation.
pub fn measure(buffer: &cosmic_text::Buffer) -> (Size, bool) {
    let mut width = 0.0_f32;
    let mut height = 0.0_f32;
    let mut has_rtl = false;
    let mut last_line_i: Option<usize> = None;

    let mut first_top_overflow = 0.0_f32;
    let mut last_bottom_overflow = 0.0_f32;
    let mut is_first_run = true;

    for run in buffer.layout_runs() {
        if is_first_run {
            // Glyph top relative to line_top, negative if ascenders
            // extend above the line slot.
            let glyph_above_line = run.line_top - (run.line_y - run.max_ascent);
            first_top_overflow = glyph_above_line.max(0.0);
            is_first_run = false;
        }
        if last_line_i != Some(run.line_i) {
            // Closed-out previous buffer line: add its margin_bottom.
            if let Some(prev_i) = last_line_i
                && let Some(line) = buffer.lines.get(prev_i)
            {
                height += line.margin_bottom();
            }
            // Opening a new buffer line: add its margin_top.
            if let Some(line) = buffer.lines.get(run.line_i) {
                height += line.margin_top();
            }
            last_line_i = Some(run.line_i);
        }
        width = width.max(run.line_w);
        height += run.line_height;
        has_rtl = has_rtl || run.rtl;

        // Track this run's bottom overflow; the loop's final
        // assignment is the *last* run's overflow.
        let glyph_below_line = (run.line_y + run.max_descent) - (run.line_top + run.line_height);
        last_bottom_overflow = glyph_below_line.max(0.0);
    }

    // Close out the very last laid-out buffer line.
    if let Some(last_i) = last_line_i
        && let Some(line) = buffer.lines.get(last_i)
    {
        height += line.margin_bottom();
    }

    height += first_top_overflow + last_bottom_overflow;

    (Size::new(width, height), has_rtl)
}

/// Returns how many logical pixels the first visible run's glyph top
/// extends above its `line_top`, i.e. the amount the renderer needs
/// to shift the buffer's `(0, 0)` *down* so that the topmost ascender
/// lands at the editor's content-area top rather than clipping
/// against the scissor.
///
/// Pairs with [`measure`]: `measure` includes this overflow in the
/// returned height; the widget reads `visual_top_pad` to know how
/// much of that height belongs above the buffer's origin.
///
/// Returns `0.0` when there's no first run (empty buffer) or when
/// the line slot fully contains the glyph ascent (the common case
/// for sensible line heights).
pub fn visual_top_pad(buffer: &cosmic_text::Buffer) -> f32 {
    let Some(first) = buffer.layout_runs().next() else {
        return 0.0;
    };
    let glyph_top = first.line_y - first.max_ascent;
    (first.line_top - glyph_top).max(0.0)
}

/// Returns how many logical pixels the LAST visible run's glyph bottom
/// extends below its slot bottom (`line_top + line_height`), i.e. the
/// amount of vertical room the widget must reserve at the bottom for
/// the descenders not to be clipped by the scissor.
///
/// Pairs with [`measure`]: `measure` includes this overflow in the
/// returned height. Widgets that allocate `min_bounds().height` will
/// already have room; widgets that pass a fixed viewport to
/// [`crate::core::text::rich_editor::Editor::update`] can read this to
/// extend their clip rect on the bottom by the same amount the top
/// is extended via [`visual_top_pad`].
///
/// Returns `0.0` when there's no last run (empty buffer) or when the
/// last line's slot fully contains its glyph descent.
pub fn visual_bottom_pad(buffer: &cosmic_text::Buffer) -> f32 {
    let Some(last) = buffer.layout_runs().last() else {
        return 0.0;
    };
    let glyph_bottom = last.line_y + last.max_descent;
    let slot_bottom = last.line_top + last.line_height;
    (glyph_bottom - slot_bottom).max(0.0)
}

/// Aligns the given [`cosmic_text::Buffer`] with the given [`Alignment`]
/// and returns its minimum [`Size`].
pub fn align(
    buffer: &mut cosmic_text::Buffer,
    font_system: &mut cosmic_text::FontSystem,
    alignment: Alignment,
) -> Size {
    let (min_bounds, has_rtl) = measure(buffer);
    let mut needs_relayout = has_rtl;

    if let Some(align) = to_align(alignment) {
        let has_multiple_lines = buffer.lines.len() > 1
            || buffer
                .lines
                .first()
                .is_some_and(|line| line.layout_opt().is_some_and(|layout| layout.len() > 1));

        if has_multiple_lines {
            for line in &mut buffer.lines {
                let _ = line.set_align(Some(align));
            }

            needs_relayout = true;
        } else if let Some(line) = buffer.lines.first_mut() {
            needs_relayout |= line.set_align(None);
        }
    }

    // TODO: Avoid relayout with some changes to `cosmic-text` (?)
    if needs_relayout {
        log::trace!("Relayouting paragraph...");

        buffer.set_size(Some(min_bounds.width), Some(min_bounds.height));
        buffer.shape_until_scroll(font_system, false);
    }

    min_bounds
}

/// Returns the attributes of the given [`Font`].
pub fn to_attributes(
    font: Font,
    letter_spacing: Em,
    font_features: &[font::Feature],
    font_variations: &[font::Variation],
) -> cosmic_text::Attrs<'static> {
    let mut attrs = cosmic_text::Attrs::new()
        .family(to_family(font.family))
        .weight(to_weight(font.weight))
        .stretch(to_stretch(font.stretch))
        .style(to_style(font.style))
        .optical_size(match font.optical_size {
            font::OpticalSize::Auto => cosmic_text::OpticalSize::Auto,
            font::OpticalSize::Fixed(bits) => cosmic_text::OpticalSize::Fixed(f32::from_bits(bits)),
            font::OpticalSize::None => cosmic_text::OpticalSize::None,
        });

    if letter_spacing.0 != 0.0 {
        attrs = attrs.letter_spacing(letter_spacing.0);
    }

    if !font_features.is_empty() {
        let mut features = cosmic_text::FontFeatures::new();
        for f in font_features {
            let _ = features.set(cosmic_text::FeatureTag::new(&f.tag.0), f.value);
        }
        attrs = attrs.font_features(features);
    }

    if !font_variations.is_empty() {
        let mut variations = cosmic_text::FontVariations::new();
        for v in font_variations {
            let _ = variations.set(cosmic_text::FeatureTag::new(&v.tag.0), v.value());
        }
        attrs = attrs.font_variations(variations);
    }

    attrs
}

fn to_family(family: font::Family) -> cosmic_text::Family<'static> {
    match family {
        font::Family::Name(name) => cosmic_text::Family::Name(name),
        font::Family::SansSerif => cosmic_text::Family::SansSerif,
        font::Family::Serif => cosmic_text::Family::Serif,
        font::Family::Cursive => cosmic_text::Family::Cursive,
        font::Family::Fantasy => cosmic_text::Family::Fantasy,
        font::Family::Monospace => cosmic_text::Family::Monospace,
    }
}

fn to_weight(weight: font::Weight) -> cosmic_text::Weight {
    match weight {
        font::Weight::Thin => cosmic_text::Weight::THIN,
        font::Weight::ExtraLight => cosmic_text::Weight::EXTRA_LIGHT,
        font::Weight::Light => cosmic_text::Weight::LIGHT,
        font::Weight::Normal => cosmic_text::Weight::NORMAL,
        font::Weight::Medium => cosmic_text::Weight::MEDIUM,
        font::Weight::Semibold => cosmic_text::Weight::SEMIBOLD,
        font::Weight::Bold => cosmic_text::Weight::BOLD,
        font::Weight::ExtraBold => cosmic_text::Weight::EXTRA_BOLD,
        font::Weight::Black => cosmic_text::Weight::BLACK,
    }
}

fn to_stretch(stretch: font::Stretch) -> cosmic_text::Stretch {
    match stretch {
        font::Stretch::UltraCondensed => cosmic_text::Stretch::UltraCondensed,
        font::Stretch::ExtraCondensed => cosmic_text::Stretch::ExtraCondensed,
        font::Stretch::Condensed => cosmic_text::Stretch::Condensed,
        font::Stretch::SemiCondensed => cosmic_text::Stretch::SemiCondensed,
        font::Stretch::Normal => cosmic_text::Stretch::Normal,
        font::Stretch::SemiExpanded => cosmic_text::Stretch::SemiExpanded,
        font::Stretch::Expanded => cosmic_text::Stretch::Expanded,
        font::Stretch::ExtraExpanded => cosmic_text::Stretch::ExtraExpanded,
        font::Stretch::UltraExpanded => cosmic_text::Stretch::UltraExpanded,
    }
}

fn to_style(style: font::Style) -> cosmic_text::Style {
    match style {
        font::Style::Normal => cosmic_text::Style::Normal,
        font::Style::Italic => cosmic_text::Style::Italic,
        font::Style::Oblique => cosmic_text::Style::Oblique,
    }
}

/// Converts an [`Alignment`] to a [`cosmic_text::Align`].
pub fn to_align(alignment: Alignment) -> Option<cosmic_text::Align> {
    match alignment {
        Alignment::Default => None,
        Alignment::Left => Some(cosmic_text::Align::Left),
        Alignment::Center => Some(cosmic_text::Align::Center),
        Alignment::Right => Some(cosmic_text::Align::Right),
        Alignment::Justified => Some(cosmic_text::Align::Justified),
    }
}

/// Converts some [`Shaping`] strategy to a [`cosmic_text::Shaping`] strategy.
pub fn to_shaping(shaping: Shaping, text: &str, has_features: bool) -> cosmic_text::Shaping {
    match shaping {
        Shaping::Auto => {
            if has_features || !text.is_ascii() {
                cosmic_text::Shaping::Advanced
            } else {
                cosmic_text::Shaping::Basic
            }
        }
        Shaping::Basic => cosmic_text::Shaping::Basic,
        Shaping::Advanced => cosmic_text::Shaping::Advanced,
    }
}

/// Converts some [`Wrapping`] strategy to a [`cosmic_text::Wrap`] strategy.
pub fn to_wrap(wrapping: Wrapping) -> cosmic_text::Wrap {
    match wrapping {
        Wrapping::None => cosmic_text::Wrap::None,
        Wrapping::Word => cosmic_text::Wrap::Word,
        Wrapping::Glyph => cosmic_text::Wrap::Glyph,
        Wrapping::WordOrGlyph => cosmic_text::Wrap::WordOrGlyph,
    }
}

/// Converts some [`Ellipsis`] strategy to a [`cosmic_text::Ellipsize`] strategy.
pub fn to_ellipsize(ellipsis: Ellipsis, max_height: f32) -> cosmic_text::Ellipsize {
    let limit = cosmic_text::EllipsizeHeightLimit::Height(max_height);

    match ellipsis {
        Ellipsis::None => cosmic_text::Ellipsize::None,
        Ellipsis::Start => cosmic_text::Ellipsize::Start(limit),
        Ellipsis::Middle => cosmic_text::Ellipsize::Middle(limit),
        Ellipsis::End => cosmic_text::Ellipsize::End(limit),
    }
}

/// Converts some [`Color`] to a [`cosmic_text::Color`].
pub fn to_color(color: Color) -> cosmic_text::Color {
    let [r, g, b, a] = color.into_rgba8();

    cosmic_text::Color::rgba(r, g, b, a)
}

/// Returns the ideal hint factor given the size and scale factor of some text.
pub fn hint_factor(_size: Pixels, _scale_factor: Option<f32>) -> Option<f32> {
    // TODO: Fix hinting in `cosmic-text`
    // const MAX_HINTING_SIZE: f32 = 18.0;

    // let hint_factor = scale_factor?;

    // if size.0 * hint_factor < MAX_HINTING_SIZE {
    //     Some(hint_factor)
    // } else {
    //     None
    // }

    None // Disable all text hinting for now
}

/// A text renderer coupled to `iced_graphics`.
pub trait Renderer {
    /// Draws the given [`Raw`] text.
    fn fill_raw(&mut self, raw: Raw);
}
