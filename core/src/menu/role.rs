/// A platform role assigned to a top-level native menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Registers the menu as the macOS Window menu.
    ///
    /// AppKit maintains the list of open windows beneath the application's items.
    Window,
    /// Registers the menu as the macOS Help menu, including its search field.
    Help,
}
