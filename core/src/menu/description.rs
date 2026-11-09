//! Message-independent presentation of native menus.
//!
//! Descriptions can be inspected without invoking actions or mapping functions.
//! A runtime pairs a description with the [`Dispatch`](super::Dispatch) returned
//! by the same menu or bar; entry paths must never be applied to a newer dispatch.
mod action;
mod entry;
mod item;
mod menu;

pub use action::Action;
pub use entry::Entry;
pub use item::Item;
pub use menu::Menu;
