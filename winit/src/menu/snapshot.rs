use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::Proxy;
use crate::core::{menu, window};
use crate::runtime::Action;

use super::branch::Branch;
use super::scope::Scope;

pub struct Snapshot {
    pub roots: Vec<Branch>,
}

impl Snapshot {
    pub fn claim_shortcuts(&mut self, inherited: &mut HashSet<menu::Shortcut>) {
        for root in &mut self.roots {
            root.claim_shortcuts(inherited);
        }
    }

    pub fn from_bar<Message: 'static>(
        bar: menu::Bar<Message>,
        window: Option<(window::Id, &Arc<winit::window::Window>)>,
        proxy: &Proxy<Message>,
        focus: crate::window::Focus,
    ) -> Self {
        let scope = Rc::new(Scope::capture(window, focus));
        let proxy = proxy.clone();
        let send: Rc<dyn Fn(Action<Message>)> = Rc::new(move |action| proxy.send_action(action));
        Self {
            roots: bar
                .menus()
                .iter()
                .map(|menu| Branch::from_menu(menu, true, &scope, &send))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;
