use iced_winit::core::{image, menu};
use objc2_foundation::NSSize;

use super::image::Image;

pub fn run() {
    let mut cache = Image::default();
    let handle = image::Handle::from_rgba(32, 16, vec![255_u8; 32 * 16 * 4]);
    let template = menu::Icon::new(&handle).template(true);
    let first = cache.update(Some(template.clone())).unwrap();
    assert_eq!(first.size(), NSSize::new(16.0, 8.0));
    assert!(first.isTemplate());
    let first = std::ptr::from_ref(first);
    assert_eq!(
        first,
        std::ptr::from_ref(cache.update(Some(template)).unwrap()),
        "an unchanged handle must reuse the decoded native image"
    );
    assert!(!cache.update(Some(handle.into())).unwrap().isTemplate());
    assert!(cache.update(None).is_none());

    let bytes = include_bytes!("../../../examples/tour/images/ferris.png");
    let encoded = image::Handle::from_bytes(bytes.as_slice());
    let image = cache.update(Some(encoded.into())).unwrap();
    assert!(image.isValid());
    assert_eq!(image.size().width.max(image.size().height), 16.0);

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/tour/images/ferris.png"
    );
    assert!(cache.update(Some(menu::Icon::new(path))).unwrap().isValid());

    for handle in [
        image::Handle::from_bytes(&b"not an image"[..]),
        image::Handle::from_path("/iced-menu-test/nonexistent.png"),
        image::Handle::from_rgba(0, 1, Vec::new()),
        image::Handle::from_rgba(1, 1, vec![0_u8; 3]),
        image::Handle::from_rgba(u32::MAX, u32::MAX, Vec::new()),
    ] {
        assert!(cache.update(Some(handle.into())).is_none());
    }
    println!(
        "native menu icons: loading, templates, sizing, caching, removal and invalid data passed"
    );
}
