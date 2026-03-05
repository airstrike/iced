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
}

impl Font {
    /// A non-monospaced sans-serif font with normal [`Weight`].
    pub const DEFAULT: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Normal,
        stretch: Stretch::Normal,
        style: Style::Normal,
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

/// A 4-byte OpenType feature tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag(pub [u8; 4]);

impl Tag {
    /// Creates a new [`Tag`] from a 4-byte array.
    pub const fn new(tag: &[u8; 4]) -> Self {
        Self(*tag)
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
    /// Creates a new [`Feature`] with the given [`Tag`] and value.
    pub const fn new(tag: Tag, value: u32) -> Self {
        Self { tag, value }
    }

    /// Creates a [`Feature`] that enables the given [`Tag`].
    pub const fn on(tag: Tag) -> Self {
        Self::new(tag, 1)
    }

    /// Creates a [`Feature`] that disables the given [`Tag`].
    pub const fn off(tag: Tag) -> Self {
        Self::new(tag, 0)
    }
}
