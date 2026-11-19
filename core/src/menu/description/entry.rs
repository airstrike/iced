use super::{Item, Menu};

/// The presentation of a native menu entry, without its message factory.
#[derive(Debug, Clone)]
pub enum Entry {
    /// An actionable or disabled item.
    Item(Item),
    /// A nested menu.
    Submenu(Menu),
    /// A separator between entries.
    Separator,
}
