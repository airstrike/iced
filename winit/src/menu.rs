//! Native application menus owned by the event-loop instance.

mod manager;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;

#[cfg(not(target_os = "macos"))]
mod other;

#[cfg(target_os = "macos")]
mod binding;
#[cfg(target_os = "macos")]
mod branch;
#[cfg(target_os = "macos")]
mod callback;
#[cfg(target_os = "macos")]
mod scope;
#[cfg(target_os = "macos")]
mod snapshot;
#[cfg(target_os = "macos")]
mod tracking;

pub use manager::Manager;
