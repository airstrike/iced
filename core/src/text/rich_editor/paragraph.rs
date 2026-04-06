use crate::text::LineHeight;
use crate::Color;

use super::span;

/// Paragraph-level formatting style.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    /// Character defaults for the paragraph.
    pub style: span::Style,
    /// Text alignment.
    pub alignment: Option<crate::text::Alignment>,
    /// Spacing after the paragraph in logical pixels.
    pub spacing_after: Option<f32>,
    /// Line height override for this paragraph.
    pub line_height: Option<LineHeight>,
    /// Line spacing within the paragraph.
    pub line_spacing: Option<Spacing>,
    /// Space before paragraph in logical pixels.
    pub space_before: Option<f32>,
    /// Nesting depth (0-8).
    pub level: u8,
    /// List marker style.
    pub list: Option<List>,
    /// Paragraph indentation.
    pub indent: Indent,
    /// Background fill for the paragraph.
    pub fill: Option<Fill>,
    /// Paragraph borders. Boxed for memory efficiency (most paragraphs have no borders).
    pub borders: Option<Box<Borders>>,
}

/// Geometry of the first visual line of a paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// Y offset from the top of the buffer to the top of this line.
    pub line_top: f32,
    /// Total height of this line (ascent + descent + leading).
    pub line_height: f32,
    /// Y offset from the top of the buffer to the baseline.
    pub baseline_y: f32,
    /// X offset of the line start (margin + alignment).
    pub x_offset: f32,
}

/// Line spacing within a paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spacing {
    /// Multiplier: 1.0 = single, 1.5, 2.0, etc.
    Multiple(f32),
    /// Fixed spacing in logical pixels.
    Exact(f32),
}

/// Paragraph indentation in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Indent {
    /// Left margin in logical pixels.
    pub left: f32,
    /// Hanging indent in logical pixels (positive = text hangs past the bullet).
    pub hanging: f32,
}

/// List marker style for a paragraph.
#[derive(Debug, Clone, PartialEq)]

pub enum List {
    /// Unordered (bullet) list.
    Bullet(Bullet),
    /// Ordered (numbered) list.
    Ordered(Number),
}

/// Unordered list bullet variant.
#[derive(Debug, Clone, PartialEq)]

pub enum Bullet {
    /// Filled circle.
    Disc,
    /// Hollow circle.
    Circle,
    /// Filled square.
    Square,
    /// Custom character.
    Custom(char),
}

/// Ordered list numbering variant.
#[derive(Debug, Clone, PartialEq)]

pub enum Number {
    /// 1, 2, 3, ...
    Arabic,
    /// a, b, c, ...
    LowerAlpha,
    /// A, B, C, ...
    UpperAlpha,
    /// i, ii, iii, ...
    LowerRoman,
    /// I, II, III, ...
    UpperRoman,
}

/// A decorative fill inside a paragraph's bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    /// The fill color.
    pub color: Color,
    /// `None` = full paragraph background (code block, callout).
    /// `Some(h)` = centered horizontal quad of height `h` (rule).
    pub height: Option<f32>,
}

/// A single border edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    /// The border color.
    pub color: Color,
    /// The border width in logical pixels.
    pub width: f32,
}

/// Four optional paragraph borders.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Borders {
    /// Top border.
    pub top: Option<Border>,
    /// Right border.
    pub right: Option<Border>,
    /// Bottom border.
    pub bottom: Option<Border>,
    /// Left border.
    pub left: Option<Border>,
}
