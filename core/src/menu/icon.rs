use crate::image;

/// An image displayed beside a native menu item.
///
/// Images are fitted within 16 × 16 points, preserving their aspect ratio.
/// Paths and encoded bytes are decoded by the operating system. Invalid images
/// are omitted. Reuse handles between menu updates to avoid decoding them again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    handle: image::Handle,
    template: bool,
}

impl Icon {
    /// Creates a full-color icon from an iced image handle or path.
    pub fn new(handle: impl Into<image::Handle>) -> Self {
        Self {
            handle: handle.into(),
            template: false,
        }
    }

    /// Uses the image's alpha channel as a mask tinted by the operating system.
    ///
    /// Template icons adapt to the menu's appearance and selection state.
    pub fn template(mut self, template: bool) -> Self {
        self.template = template;
        self
    }

    /// Returns the image handle.
    pub fn handle(&self) -> &image::Handle {
        &self.handle
    }

    /// Returns whether the operating system tints this image as a template.
    pub fn is_template(&self) -> bool {
        self.template
    }
}

impl From<image::Handle> for Icon {
    fn from(handle: image::Handle) -> Self {
        Self::new(handle)
    }
}

impl From<&image::Handle> for Icon {
    fn from(handle: &image::Handle) -> Self {
        Self::new(handle)
    }
}

impl From<&Icon> for Icon {
    fn from(icon: &Icon) -> Self {
        icon.clone()
    }
}
