//! Convert between [`cosmic_text`] and [`crate::core`] types.
use crate::core::font::{self, Font};
use crate::core::text::editor::{LineEnding, Motion};
use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use crate::core::{Color, Em, Padding};

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
        .optical_size(to_optical_size(font.optical_size));

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

/// Converts some [`font::Family`] to a [`cosmic_text::Family`].
pub fn to_family(family: font::Family) -> cosmic_text::Family<'static> {
    match family {
        font::Family::Name(name) => cosmic_text::Family::Name(name),
        font::Family::SansSerif => cosmic_text::Family::SansSerif,
        font::Family::Serif => cosmic_text::Family::Serif,
        font::Family::Cursive => cosmic_text::Family::Cursive,
        font::Family::Fantasy => cosmic_text::Family::Fantasy,
        font::Family::Monospace => cosmic_text::Family::Monospace,
    }
}

/// Converts some [`cosmic_text::Family`] to a [`font::Family`].
///
/// The family name is interned by [`font::Family::name`], since
/// cosmic-text only lends it for the lifetime of the borrow.
pub fn from_family(family: cosmic_text::Family<'_>) -> font::Family {
    match family {
        cosmic_text::Family::Name(name) => font::Family::name(name),
        cosmic_text::Family::SansSerif => font::Family::SansSerif,
        cosmic_text::Family::Serif => font::Family::Serif,
        cosmic_text::Family::Cursive => font::Family::Cursive,
        cosmic_text::Family::Fantasy => font::Family::Fantasy,
        cosmic_text::Family::Monospace => font::Family::Monospace,
    }
}

/// Converts some [`font::Weight`] to a [`cosmic_text::Weight`].
pub fn to_weight(weight: font::Weight) -> cosmic_text::Weight {
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

/// Converts some [`font::Stretch`] to a [`cosmic_text::Stretch`].
pub fn to_stretch(stretch: font::Stretch) -> cosmic_text::Stretch {
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

/// Converts some [`font::Style`] to a [`cosmic_text::Style`].
pub fn to_style(style: font::Style) -> cosmic_text::Style {
    match style {
        font::Style::Normal => cosmic_text::Style::Normal,
        font::Style::Italic => cosmic_text::Style::Italic,
        font::Style::Oblique => cosmic_text::Style::Oblique,
    }
}

/// Converts some [`font::OpticalSize`] to a [`cosmic_text::OpticalSize`].
pub fn to_optical_size(optical_size: font::OpticalSize) -> cosmic_text::OpticalSize {
    match optical_size {
        font::OpticalSize::Auto => cosmic_text::OpticalSize::Auto,
        font::OpticalSize::Fixed(bits) => cosmic_text::OpticalSize::Fixed(f32::from_bits(bits)),
        font::OpticalSize::None => cosmic_text::OpticalSize::None,
    }
}

/// Converts some [`cosmic_text::OpticalSize`] to a [`font::OpticalSize`].
pub fn from_optical_size(optical_size: cosmic_text::OpticalSize) -> font::OpticalSize {
    match optical_size {
        cosmic_text::OpticalSize::Auto => font::OpticalSize::Auto,
        cosmic_text::OpticalSize::Fixed(v) => font::OpticalSize::Fixed(v.to_bits()),
        cosmic_text::OpticalSize::None => font::OpticalSize::None,
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

/// Converts a [`cosmic_text::Align`] to an [`Alignment`].
pub fn from_align(align: cosmic_text::Align) -> Alignment {
    match align {
        cosmic_text::Align::Left => Alignment::Left,
        cosmic_text::Align::Center => Alignment::Center,
        cosmic_text::Align::Right => Alignment::Right,
        cosmic_text::Align::Justified => Alignment::Justified,
        cosmic_text::Align::End => Alignment::Default,
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

/// Converts some [`cosmic_text::Color`] to a [`Color`].
pub fn from_color(color: cosmic_text::Color) -> Color {
    Color::from_rgba8(color.r(), color.g(), color.b(), color.a() as f32 / 255.0)
}

/// Extracts the font size of some [`cosmic_text::CacheMetrics`].
pub fn from_metrics(metrics: cosmic_text::CacheMetrics) -> f32 {
    let metrics: cosmic_text::Metrics = metrics.into();

    metrics.font_size
}

/// Converts some [`cosmic_text::SpanPadding`] to a [`Padding`].
pub fn from_padding(padding: cosmic_text::SpanPadding) -> Padding {
    Padding {
        top: padding.top(),
        bottom: padding.bottom(),
        left: padding.start(),
        right: padding.end(),
    }
}

/// Converts some [`Motion`] to a [`cosmic_text::Motion`].
pub fn to_motion(motion: Motion) -> cosmic_text::Motion {
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

/// Converts some [`cosmic_text::LineEnding`] to a [`LineEnding`].
pub fn from_line_ending(ending: cosmic_text::LineEnding) -> LineEnding {
    match ending {
        cosmic_text::LineEnding::Lf => LineEnding::Lf,
        cosmic_text::LineEnding::CrLf => LineEnding::CrLf,
        cosmic_text::LineEnding::Cr => LineEnding::Cr,
        cosmic_text::LineEnding::LfCr => LineEnding::LfCr,
        cosmic_text::LineEnding::None => LineEnding::None,
    }
}
