//! Native application menus.
//!
//! Build a [`Bar`] from menus and return it from your program's menu function.
//! Menu actions are evaluated only when the user activates an enabled item.
//! Native menus are currently supported on macOS; requesting a nonempty bar on
//! another platform is not yet implemented. An empty bar preserves the platform's
//! default application menu.

mod action;
mod bar;
mod construction;
mod entry;
mod item;
#[expect(
    clippy::module_inception,
    reason = "The private leaf holds Menu; this module is its public facade."
)]
mod menu;
pub mod shortcut;

pub use action::Action;
pub use bar::Bar;
pub use construction::{bar, edit, item, separator, submenu};
pub use entry::Entry;
pub use item::Item;
pub use menu::Menu;
pub use shortcut::Shortcut;
