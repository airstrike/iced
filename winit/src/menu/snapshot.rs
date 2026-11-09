use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::Proxy;
use crate::core::{menu, window};
use crate::runtime::Action;

use super::binding::Binding;
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
        let (menus, dispatch) = bar.into_parts();
        let binding = Rc::new(Binding::new(dispatch, scope, send));
        Self {
            roots: menus
                .iter()
                .enumerate()
                .map(|(index, menu)| Branch::from_menu(menu, true, &binding, &mut vec![index]))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;
