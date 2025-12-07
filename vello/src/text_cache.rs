//! Text caching for the Vello renderer.
//!
//! This module provides caching for text layouts to avoid re-creating
//! Parley layouts on every frame, dramatically improving performance.

use crate::core::text::{Alignment, LineHeight, Shaping, Wrapping};
use crate::core::{Font, Pixels, Size};
use crate::text::Paragraph;

use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};

/// A key for caching text layouts.
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    /// The content of the text.
    pub content: String,
    /// The font size.
    pub size: Pixels,
    /// The line height.
    pub line_height: LineHeight,
    /// The font.
    pub font: Font,
    /// The horizontal alignment.
    pub align_x: Alignment,
    /// The vertical alignment.
    pub align_y: crate::core::alignment::Vertical,
    /// The bounds for layout.
    pub bounds: Size,
    /// The text shaping strategy.
    pub shaping: Shaping,
    /// The text wrapping strategy.
    pub wrapping: Wrapping,
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.content.hash(state);
        self.size.0.to_bits().hash(state);

        match self.line_height {
            LineHeight::Relative(factor) => {
                0u8.hash(state);
                factor.to_bits().hash(state);
            }
            LineHeight::Absolute(pixels) => {
                1u8.hash(state);
                pixels.0.to_bits().hash(state);
            }
        }

        self.font.family.hash(state);
        self.font.weight.hash(state);
        self.font.stretch.hash(state);
        self.font.style.hash(state);

        (self.align_x as u8).hash(state);
        (self.align_y as u8).hash(state);

        self.bounds.width.to_bits().hash(state);
        self.bounds.height.to_bits().hash(state);

        (self.shaping as u8).hash(state);
        (self.wrapping as u8).hash(state);
    }
}

impl Eq for Key {}

/// A cache entry containing a paragraph.
#[derive(Clone)]
pub struct Entry {
    /// The cached paragraph.
    pub paragraph: Paragraph,
}

/// A cache for text layouts.
#[derive(Clone)]
pub struct Cache {
    /// The internal storage for cached paragraphs.
    storage: FxHashMap<u64, Entry>,
}

impl Cache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self {
            storage: FxHashMap::default(),
        }
    }

    /// Allocates a paragraph from the cache or creates a new one.
    pub fn allocate(&mut self, key: Key) -> &Paragraph {
        use std::hash::Hash;
        use std::hash::Hasher;

        // Generate hash key using FxHasher
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // Check if we have this cached
        let entry = self.storage.entry(hash).or_insert_with(|| {
            // Create new paragraph with the text
            use crate::core::text::Paragraph as _;

            let text = crate::core::Text {
                content: key.content.as_str(),
                bounds: key.bounds,
                size: key.size,
                font: key.font,
                line_height: key.line_height,
                align_x: key.align_x,
                align_y: key.align_y,
                wrapping: key.wrapping,
                shaping: key.shaping,
            };

            let paragraph = Paragraph::with_text(text);
            Entry { paragraph }
        });

        &entry.paragraph
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.storage.clear();
    }

    /// Trims the cache to remove unused entries.
    pub fn trim(&mut self, max_entries: usize) {
        if self.storage.len() > max_entries {
            // TODO: Explore LRU or other eviction strategies
            self.clear();
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}
