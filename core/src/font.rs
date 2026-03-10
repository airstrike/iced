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

    /// Creates a [`Font`] with the given [`Family::Name`] from a runtime string.
    ///
    /// The name is interned in a global cache and never freed.
    /// For compile-time constants, prefer [`Font::with_name`].
    pub fn from_name(name: &str) -> Self {
        use rustc_hash::FxHashSet;
        use std::sync::{LazyLock, Mutex};

        static NAMES: LazyLock<Mutex<FxHashSet<&'static str>>> =
            LazyLock::new(|| Mutex::new(FxHashSet::default()));

        let mut set = NAMES.lock().expect("font name pool");
        let interned = if let Some(&existing) = set.get(name) {
            existing
        } else {
            let s: &'static str = name.to_owned().leak();
            let _ = set.insert(s);
            s
        };

        Font {
            family: Family::Name(interned),
            ..Self::DEFAULT
        }
    }
}

/// Creates a [`Font`] with the given [`Family::Name`] from a runtime string.
///
/// Shorthand for [`Font::from_name`].
pub fn font(name: &str) -> Font {
    Font::from_name(name)
}

impl From<&str> for Font {
    fn from(name: &str) -> Self {
        Font::from_name(name)
    }
}

impl From<Option<&str>> for Font {
    fn from(name: Option<&str>) -> Self {
        match name {
            Some(name) => Font::from_name(name),
            None => Font::DEFAULT,
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
