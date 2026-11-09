use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::MainThreadMarker;

/// Restores the previous Help registration only while we still own it.
pub(super) struct Registration {
    application: Retained<NSApplication>,
    previous: Option<Retained<NSMenu>>,
    installed: Option<Retained<NSMenu>>,
}

impl Registration {
    pub(super) fn new(mtm: MainThreadMarker) -> Self {
        Self {
            application: NSApplication::sharedApplication(mtm),
            previous: None,
            installed: None,
        }
    }

    pub(super) fn update(&mut self, menu: Option<&NSMenu>) {
        use objc2::Message;

        if same(self.installed.as_deref(), menu) {
            return;
        }

        let current = self.current();
        if let Some(menu) = menu {
            if self.installed.is_none() || !same(current.as_deref(), self.installed.as_deref()) {
                self.previous = current;
            }
            self.set(Some(menu));
        } else {
            if same(current.as_deref(), self.installed.as_deref()) {
                self.set(self.previous.as_deref());
            }
            self.previous = None;
        }
        self.installed = menu.map(NSMenu::retain);
    }

    fn current(&self) -> Option<Retained<NSMenu>> {
        self.application.helpMenu()
    }

    fn set(&self, menu: Option<&NSMenu>) {
        self.application.setHelpMenu(menu);
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.update(None);
    }
}

fn same(first: Option<&NSMenu>, second: Option<&NSMenu>) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => std::ptr::eq(first, second),
        (None, None) => true,
        _ => false,
    }
}

/// Window registration marks the menu itself; removing the role needs a fresh menu.
pub(super) fn window(menu: &NSMenu) {
    let app = NSApplication::sharedApplication(MainThreadMarker::from(menu));
    app.setWindowsMenu(Some(menu));
}
