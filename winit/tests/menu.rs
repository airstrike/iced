//! Main-thread tests for native menu ownership.
#[cfg(target_os = "macos")]
#[path = "../src/menu/macos/registration.rs"]
mod registration;
#[cfg(target_os = "macos")]
#[path = "menu/roles.rs"]
mod roles;

fn main() {
    // There are no ignored tests in this main-thread executable.
    if std::env::args().any(|argument| argument == "--ignored") {
        return;
    }
    #[cfg(target_os = "macos")]
    roles::run();
}
