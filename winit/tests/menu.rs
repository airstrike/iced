//! Main-thread tests for native menu ownership.
#[cfg(target_os = "macos")]
pub use iced_winit::core;
#[cfg(target_os = "macos")]
#[path = "menu/icons.rs"]
mod icons;
#[cfg(target_os = "macos")]
#[path = "../src/menu/macos/image.rs"]
#[allow(unsafe_code)]
mod image;
#[cfg(target_os = "macos")]
#[path = "../src/menu/macos/registration.rs"]
mod registration;
#[cfg(target_os = "macos")]
#[path = "menu/roles.rs"]
#[allow(unsafe_code)]
mod roles;

fn main() {
    // There are no ignored tests in this main-thread executable.
    if std::env::args().any(|argument| argument == "--ignored") {
        return;
    }
    #[cfg(target_os = "macos")]
    roles::run();
    #[cfg(target_os = "macos")]
    icons::run();
}
