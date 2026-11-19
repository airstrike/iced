use std::cell::Cell;

/// Whether the main run loop is confirmed to be in its ordinary input mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Default,
    Other,
}

/// Tracks nested AppKit menus through the end of selection dispatch.
#[derive(Default)]
pub struct Tracking {
    depth: Cell<usize>,
    unwinding: Cell<bool>,
}

impl Tracking {
    pub fn begin(&self) {
        self.depth.set(self.depth.get() + 1);
    }

    pub fn end(&self) {
        self.depth.set(self.depth.get().saturating_sub(1));
        // AppKit may call menuDidClose before the selected item's action.
        self.unwinding.set(true);
    }

    pub fn is_frozen(&self, mode: Mode) -> bool {
        mode != Mode::Default || self.depth.get() > 0 || self.unwinding.get()
    }

    pub fn resume(&self, mode: Mode) {
        if mode == Mode::Default {
            self.unwinding.set(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Mode, Tracking};

    #[test]
    fn closing_stays_frozen_until_selection_dispatch_unwinds() {
        let tracking = Tracking::default();
        tracking.begin();
        tracking.begin();
        tracking.end();
        tracking.resume(Mode::Default);
        assert!(tracking.is_frozen(Mode::Default));
        tracking.end();
        assert!(tracking.is_frozen(Mode::Default));
        tracking.resume(Mode::Default);
        assert!(!tracking.is_frozen(Mode::Default));
    }

    #[test]
    fn inherited_menus_and_modal_loops_freeze_without_delegate_callbacks() {
        let tracking = Tracking::default();
        assert!(tracking.is_frozen(Mode::Other));
        tracking.resume(Mode::Other);
        assert!(tracking.is_frozen(Mode::Other));
        assert!(!tracking.is_frozen(Mode::Default));
    }

    #[test]
    fn nested_flush_cannot_clear_selection_unwinding() {
        let tracking = Tracking::default();
        tracking.begin();
        tracking.end();
        tracking.resume(Mode::Other);
        assert!(tracking.is_frozen(Mode::Default));
        tracking.resume(Mode::Default);
        assert!(!tracking.is_frozen(Mode::Default));
    }

    #[test]
    fn default_mode_during_activation_does_not_thaw_presentation() {
        let tracking = Tracking::default();
        tracking.begin();
        tracking.resume(Mode::Default);
        assert!(tracking.is_frozen(Mode::Default));
        tracking.end();
        assert!(tracking.is_frozen(Mode::Default));
        tracking.resume(Mode::Default);
        assert!(!tracking.is_frozen(Mode::Default));
    }
}
