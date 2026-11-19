use std::rc::Rc;

use crate::core::{menu, window};
use crate::runtime::Action;

use super::callback::Callback;
use super::scope::Scope;

/// Retains the originating dispatch for every native target in one snapshot.
pub struct Binding<Message> {
    dispatch: menu::Dispatch<Message>,
    scope: Rc<Scope>,
    send: Rc<dyn Fn(Action<Message>)>,
}

impl<Message: 'static> Binding<Message> {
    pub fn new(
        dispatch: menu::Dispatch<Message>,
        scope: Rc<Scope>,
        send: Rc<dyn Fn(Action<Message>)>,
    ) -> Self {
        Self {
            dispatch,
            scope,
            send,
        }
    }

    pub fn window(&self) -> Option<window::Id> {
        self.scope.window
    }

    pub fn callback(self: &Rc<Self>, path: Vec<usize>) -> Rc<Callback> {
        let binding = Rc::clone(self);
        Rc::new(Callback::new(
            Rc::clone(&self.scope),
            move || match binding.dispatch.activate(&path) {
                Some(menu::Output::Message(message)) => (binding.send)(Action::Output(message)),
                Some(menu::Output::Edit(command)) => {
                    if let Some(window) = binding.window() {
                        (binding.send)(Action::Event {
                            window,
                            event: crate::core::Event::Edit(command),
                        });
                    }
                }
                None => {}
            },
        ))
    }
}
