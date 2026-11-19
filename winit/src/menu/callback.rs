use std::cell::Cell;
use std::rc::Rc;

use super::scope::Scope;

/// An activation keeps its factory and captured window; retired targets are inert.
pub struct Callback {
    active: Cell<bool>,
    scope: Rc<Scope>,
    activate: Box<dyn Fn()>,
}

impl Callback {
    pub fn new(scope: Rc<Scope>, activate: impl Fn() + 'static) -> Self {
        Self {
            active: Cell::new(true),
            scope,
            activate: Box::new(activate),
        }
    }

    pub fn invoke(&self) {
        if self.active.get() && (self.scope.is_current)() {
            (self.activate)();
        }
    }

    pub fn retire(&self) {
        self.active.set(false);
    }
}
