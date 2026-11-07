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

    /// Creates a [`Font`] with the given [`Family::Name`] and default attributes.
    pub const fn new(name: &'static str) -> Self {
        Self {
            family: Family::Name(name),
            ..Self::DEFAULT
        }
    }

    /// Creates a [`Font`] with the given [`Family`] and default attributes.
    pub fn with_family(family: impl Into<Family>) -> Self {
        Font {
            family: family.into(),
            ..Self::DEFAULT
        }
    }

    /// Sets the [`Weight`] of the [`Font`].
    pub const fn weight(self, weight: Weight) -> Self {
        Self { weight, ..self }
    }

    /// Sets the [`Stretch`] of the [`Font`].
    pub const fn stretch(self, stretch: Stretch) -> Self {
        Self { stretch, ..self }
    }

    /// Sets the [`Style`] of the [`Font`].
    pub const fn style(self, style: Style) -> Self {
        Self { style, ..self }
    }
}

impl From<&'static str> for Font {
    fn from(name: &'static str) -> Self {
        Font::new(name)
    }
}

impl From<Family> for Font {
    fn from(family: Family) -> Self {
        Font::with_family(family)
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

impl Family {
    /// A list of all the different standalone family variants.
    pub const VARIANTS: &[Self] = &[
        Self::Serif,
        Self::SansSerif,
        Self::Cursive,
        Self::Fantasy,
        Self::Monospace,
    ];

    /// Creates a [`Family::Name`] from the given string.
    ///
    /// The name is interned in a global cache and never freed.
    pub fn name(name: &str) -> Self {
        use rustc_hash::FxHashSet;
        use std::sync::{LazyLock, Mutex};

        static NAMES: LazyLock<Mutex<FxHashSet<&'static str>>> = LazyLock::new(Mutex::default);

        let mut names = NAMES.lock().expect("lock font name cache");

        let Some(name) = names.get(name) else {
            let name: &'static str = name.to_owned().leak();
            let _ = names.insert(name);

            return Self::Name(name);
        };

        Self::Name(name)
    }
}

impl From<&str> for Family {
    fn from(name: &str) -> Self {
        Family::name(name)
    }
}

impl std::fmt::Display for Family {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Family::Name(name) => name,
            Family::Serif => "Serif",
            Family::SansSerif => "Sans-serif",
            Family::Cursive => "Cursive",
            Family::Fantasy => "Fantasy",
            Family::Monospace => "Monospace",
        })
    }
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

/// A font error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {}

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

impl From<[u8; 4]> for Tag {
    fn from(value: [u8; 4]) -> Self {
        Self(value)
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

/// A font variation axis setting for variable fonts.
///
/// The value is stored as `u32` bits internally so that
/// `Variation` can derive `PartialEq`, `Eq`, and `Hash`.
#[derive(Debug, Clone, Copy)]
pub struct Variation {
    /// The variation axis [`Tag`] (e.g. `wdth`, `slnt`, `GRAD`).
    pub tag: Tag,
    /// The axis value, stored as `f32::to_bits()`.
    pub value_bits: u32,
}

impl Variation {
    /// Creates a new [`Variation`] with the given [`Tag`] and value.
    pub fn new(tag: Tag, value: f32) -> Self {
        Self {
            tag,
            value_bits: value.to_bits(),
        }
    }

    /// Returns the `f32` value of this variation.
    pub fn value(self) -> f32 {
        f32::from_bits(self.value_bits)
    }
}

impl PartialEq for Variation {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag && self.value_bits == other.value_bits
    }
}

impl Eq for Variation {}

impl Hash for Variation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tag.hash(state);
        self.value_bits.hash(state);
    }
}
