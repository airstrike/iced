/// Per-character formatting style.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    /// Bold (font weight >= 700).
    pub bold: Option<bool>,
    /// Italic.
    pub italic: Option<bool>,
    /// Underline.
    pub underline: Option<bool>,
    /// Strikethrough.
    pub strikethrough: Option<bool>,
    /// Override font.
    pub font: Option<crate::Font>,
    /// Override font size in logical pixels.
    pub size: Option<f32>,
    /// Override text color.
    pub color: Option<crate::Color>,
    /// Override letter spacing.
    pub letter_spacing: Option<f32>,
}
