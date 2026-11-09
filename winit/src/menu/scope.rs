use std::sync::{Arc, Weak};

use crate::core::window;

pub struct Scope {
    pub(super) window: Option<window::Id>,
    pub(super) is_current: Box<dyn Fn() -> bool>,
}

impl Scope {
    pub(super) fn capture(
        window: Option<(window::Id, &Arc<winit::window::Window>)>,
        focus: crate::window::Focus,
    ) -> Self {
        match window {
            Some((id, raw)) => {
                let raw: Weak<winit::window::Window> = Arc::downgrade(raw);
                Self {
                    window: Some(id),
                    is_current: Box::new(move || {
                        focus.get() == Some(id)
                            && raw.upgrade().is_some_and(|window| window.has_focus())
                    }),
                }
            }
            None => Self {
                window: None,
                is_current: Box::new(|| true),
            },
        }
    }
}
