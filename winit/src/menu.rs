//! Native application menus owned by the event-loop instance.

mod manager;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
#[path = "menu/macos.rs"]
mod platform;

#[cfg(not(target_os = "macos"))]
#[path = "menu/other.rs"]
mod platform;

#[cfg(target_os = "macos")]
mod snapshot;
#[cfg(target_os = "macos")]
mod tracking;

pub use manager::Manager;
