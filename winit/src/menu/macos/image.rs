use objc2::rc::Retained;
use objc2_app_kit::{NSBitmapFormat, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
use objc2_foundation::{NSData, NSSize};

use crate::core::{image, menu};

/// Keeps decoding out of menu updates while the source handle is unchanged.
#[derive(Default)]
pub(super) struct Image {
    source: Option<menu::Icon>,
    native: Option<Retained<NSImage>>,
}

impl Image {
    pub(super) fn update(&mut self, source: Option<menu::Icon>) -> Option<&NSImage> {
        let unchanged = match (&self.source, &source) {
            (Some(previous), Some(next)) => {
                previous.handle().id() == next.handle().id()
                    && previous.is_template() == next.is_template()
            }
            (None, None) => true,
            _ => false,
        };

        if !unchanged {
            self.native = source.as_ref().and_then(|icon| match load(icon) {
                Ok(image) => Some(image),
                Err(error) => {
                    tracing::warn!(%error, "Could not load native menu icon");
                    None
                }
            });
            self.source = source;
        }

        self.native.as_deref()
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("could not read image: {0}")]
    Read(#[from] std::io::Error),
    #[error("the image format or dimensions are invalid")]
    InvalidImage,
    #[error("RGBA dimensions must be nonzero and match the pixel buffer")]
    InvalidPixels,
    #[error("AppKit could not allocate the image")]
    Allocation,
}

fn load(icon: &menu::Icon) -> Result<Retained<NSImage>, Error> {
    let image = match icon.handle() {
        image::Handle::Path(_, path) => decode(&std::fs::read(path)?)?,
        image::Handle::Bytes(_, bytes) => decode(bytes)?,
        image::Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } => rgba(*width, *height, pixels)?,
    };

    let size = image.size();
    if !size.width.is_finite()
        || !size.height.is_finite()
        || size.width <= 0.0
        || size.height <= 0.0
    {
        return Err(Error::InvalidImage);
    }

    let scale = 16.0 / size.width.max(size.height);
    image.setSize(NSSize::new(size.width * scale, size.height * scale));
    image.setTemplate(icon.is_template());
    Ok(image)
}

fn decode(bytes: &[u8]) -> Result<Retained<NSImage>, Error> {
    use objc2::AnyThread;

    NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes)).ok_or(Error::InvalidImage)
}

fn rgba(width: u32, height: u32, pixels: &[u8]) -> Result<Retained<NSImage>, Error> {
    use objc2::AnyThread;

    let row_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .filter(|_| width > 0 && height > 0)
        .ok_or(Error::InvalidPixels)?;
    let length = usize::try_from(height)
        .ok()
        .and_then(|height| row_bytes.checked_mul(height))
        .filter(|length| *length == pixels.len())
        .ok_or(Error::InvalidPixels)?;
    let native_width = isize::try_from(width).map_err(|_| Error::InvalidPixels)?;
    let native_height = isize::try_from(height).map_err(|_| Error::InvalidPixels)?;
    let native_row_bytes = isize::try_from(row_bytes).map_err(|_| Error::InvalidPixels)?;

    // SAFETY: A null planes pointer asks AppKit to allocate its own storage.
    // Dimensions and row size are checked above; pixels are interleaved straight
    // RGBA, with 8 bits per sample and 32 bits per pixel in device RGB space.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            native_width,
            native_height,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            NSBitmapFormat::AlphaNonpremultiplied,
            native_row_bytes,
            32,
        )
    }
    .ok_or(Error::Allocation)?;

    let destination = bitmap.bitmapData();
    if destination.is_null() {
        return Err(Error::Allocation);
    }
    // SAFETY: AppKit allocated row_bytes * height writable bytes for this bitmap.
    // The source has exactly that length and cannot overlap AppKit-owned storage.
    unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), destination, length) };

    let image = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize::new(f64::from(width), f64::from(height)),
    );
    image.addRepresentation(&bitmap);
    Ok(image)
}
