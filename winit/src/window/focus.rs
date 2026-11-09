use std::cell::Cell;
use std::rc::Rc;

use crate::core::window::Id;

/// Remembers key-window transitions without inferring focus from window age.
#[derive(Clone, Default)]
pub struct Focus {
    current: Rc<Cell<Option<Id>>>,
}

impl Focus {
    pub fn get(&self) -> Option<Id> {
        self.current.get()
    }

    pub fn update(&mut self, id: Id, focused: bool) {
        if focused {
            self.current.set(Some(id));
        } else if self.current.get() == Some(id) {
            self.current.set(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Focus;
    use crate::core::window::Id;

    #[test]
    fn late_blur_cannot_clear_another_key_window() {
        let first = Id::unique();
        let second = Id::unique();
        let mut focus = Focus::default();
        focus.update(first, true);
        focus.update(second, true);
        focus.update(first, false);
        assert_eq!(focus.get(), Some(second));
        focus.update(second, false);
        assert_eq!(focus.get(), None);
    }

    #[test]
    fn closing_key_window_does_not_choose_a_previous_window() {
        let first = Id::unique();
        let second = Id::unique();
        let mut focus = Focus::default();
        let callbacks = focus.clone();
        focus.update(first, true);
        focus.update(second, true);
        assert_eq!(callbacks.get(), Some(second));
        focus.update(second, false);
        assert_eq!(focus.get(), None);
        assert_eq!(callbacks.get(), None);
    }
}
