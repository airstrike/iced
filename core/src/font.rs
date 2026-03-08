//! Load and use fonts.
use std::hash::Hash;

/// A font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Font {
    /// The [`Family`] of the [`Font`].
    pub family: Family,
    /// The [`Weight`] of the [`Font`].
    pub weight: Weight,
    /// The [`Stretch`] of the [`Font`].
    pub stretch: Stretch,
    /// The [`Style`] of the [`Font`].
    pub style: Style,
    /// Optical size setting for variable fonts with an `opsz` axis.
    pub optical_size: OpticalSize,
}

impl Font {
    /// A non-monospaced sans-serif font with normal [`Weight`].
    pub const DEFAULT: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Normal,
        stretch: Stretch::Normal,
        style: Style::Normal,
        optical_size: if cfg!(feature = "auto-optical-size") {
            OpticalSize::Auto
        } else {
            OpticalSize::None
        },
    };

    /// A monospaced font with normal [`Weight`].
    pub const MONOSPACE: Font = Font {
        family: Family::Monospace,
        ..Self::DEFAULT
    };

    /// Creates a non-monospaced [`Font`] with the given [`Family::Name`] and
    /// normal [`Weight`].
    pub const fn with_name(name: &'static str) -> Self {
        Font {
            family: Family::Name(name),
            ..Self::DEFAULT
        }
    }
}

/// A font family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Family {
    /// The name of a font family of choice.
    Name(&'static str),

    /// Serif fonts represent the formal text style for a script.
    Serif,

    /// Glyphs in sans-serif fonts, as the term is used in CSS, are generally low
    /// contrast and have stroke endings that are plain — without any flaring,
    /// cross stroke, or other ornamentation.
    #[default]
    SansSerif,

    /// Glyphs in cursive fonts generally use a more informal script style, and
    /// the result looks more like handwritten pen or brush writing than printed
    /// letterwork.
    Cursive,

    /// Fantasy fonts are primarily decorative or expressive fonts that contain
    /// decorative or expressive representations of characters.
    Fantasy,

    /// The sole criterion of a monospace font is that all glyphs have the same
    /// fixed width.
    Monospace,
}

/// The weight of some text.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Weight {
    Thin,
    ExtraLight,
    Light,
    #[default]
    Normal,
    Medium,
    Semibold,
    Bold,
    ExtraBold,
    Black,
}

/// The width of some text.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Stretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

/// The style of some text.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// Optical size setting for variable fonts with an `opsz` axis.
///
/// The `Fixed` variant stores the value as `u32` bits internally
/// so that `OpticalSize` can derive `PartialEq`, `Eq`, and `Hash`,
/// which is required for `Font` to be usable in match patterns.
///
/// The default is [`None`](OpticalSize::None) unless the `auto-optical-size`
/// feature is enabled, in which case it defaults to [`Auto`](OpticalSize::Auto).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OpticalSize {
    /// Automatically set `opsz` to match the font size.
    #[cfg_attr(feature = "auto-optical-size", default)]
    Auto,
    /// Set `opsz` to a specific value, independent of font size.
    /// Stored as `f32::to_bits()`.
    Fixed(u32),
    /// Disable optical sizing entirely.
    #[cfg_attr(not(feature = "auto-optical-size"), default)]
    None,
}

impl OpticalSize {
    /// Creates a [`Fixed`](Self::Fixed) optical size from an `f32` value.
    pub fn fixed(value: f32) -> Self {
        Self::Fixed(value.to_bits())
    }

    /// Returns the `f32` value if this is [`Fixed`](Self::Fixed).
    pub fn value(self) -> Option<f32> {
        match self {
            Self::Fixed(bits) => Some(f32::from_bits(bits)),
            _ => Option::None,
        }
    }
}

/// A 4-byte OpenType feature tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag(pub [u8; 4]);

impl Tag {
    /// Creates a new [`Tag`] from a 4-byte array.
    pub const fn new(tag: &[u8; 4]) -> Self {
        Self(*tag)
    }
}

impl From<&[u8; 4]> for Tag {
    fn from(tag: &[u8; 4]) -> Self {
        Self(*tag)
    }
}

impl From<Tag> for Feature {
    fn from(tag: Tag) -> Self {
        Self::on(tag)
    }
}

impl From<&[u8; 4]> for Feature {
    fn from(tag: &[u8; 4]) -> Self {
        Self::on(tag)
    }
}

/// An OpenType font feature setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Feature {
    /// The feature [`Tag`].
    pub tag: Tag,
    /// The value of the feature. `1` enables, `0` disables.
    pub value: u32,
}

impl Feature {
    /// Creates a new [`Feature`] with the given tag and value.
    pub fn new(tag: impl Into<Tag>, value: u32) -> Self {
        Self {
            tag: tag.into(),
            value,
        }
    }

    /// Creates a [`Feature`] that enables the given tag.
    pub fn on(tag: impl Into<Tag>) -> Self {
        Self::new(tag, 1)
    }

    /// Creates a [`Feature`] that disables the given tag.
    pub fn off(tag: impl Into<Tag>) -> Self {
        Self::new(tag, 0)
    }
}
