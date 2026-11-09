use std::collections::HashSet;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::MainThreadMarker;

use crate::Proxy;
use crate::menu::snapshot::Snapshot;

use super::branch::Branch;
use super::delegate::Delegate;
use super::{run_loop, shortcut};

/// Owns only the roots appended to winit's existing application menu.
pub struct Menu {
    main: Option<Retained<NSMenu>>,
    roots: Vec<Branch>,
    delegate: Retained<Delegate>,
    pending: Option<Snapshot>,
}

impl Menu {
    pub fn new<Message: 'static>(proxy: Proxy<Message>) -> Self {
        let mtm = MainThreadMarker::new().expect("winit polls its instance on the main thread");
        Self {
            main: None,
            roots: Vec::new(),
            delegate: Delegate::new(mtm, proxy),
            pending: None,
        }
    }

    pub fn update(&mut self, snapshot: Snapshot) {
        if self.delegate.ivars().tracking.is_frozen(run_loop::mode()) {
            self.pending = Some(snapshot);
            self.delegate.schedule_wake();
        } else {
            self.pending = None;
            self.apply(snapshot);
        }
    }

    pub fn flush(&mut self) {
        let mode = run_loop::mode();
        self.delegate.ivars().tracking.resume(mode);
        if !self.delegate.ivars().tracking.is_frozen(mode)
            && let Some(snapshot) = self.pending.take()
        {
            self.apply(snapshot);
        }
    }

    fn apply(&mut self, mut snapshot: Snapshot) {
        if self.roots.is_empty() && snapshot.roots.is_empty() {
            return;
        }

        let mtm = MainThreadMarker::from(&*self.delegate);
        let main = self.main.get_or_insert_with(|| {
            let app = NSApplication::sharedApplication(mtm);
            app.mainMenu().unwrap_or_else(|| {
                let main = NSMenu::new(mtm);
                app.setMainMenu(Some(&main));
                main
            })
        });

        let mut shortcuts = HashSet::new();
        for item in main.itemArray() {
            if !self
                .roots
                .iter()
                .any(|root| std::ptr::eq(&*root.item, &*item))
            {
                shortcut::collect(&item, &mut shortcuts);
            }
        }
        snapshot.claim_shortcuts(&mut shortcuts);

        while self.roots.len() > snapshot.roots.len() {
            if let Some(root) = self.roots.pop() {
                main.removeItem(&root.item);
            }
        }

        for (index, branch) in snapshot.roots.into_iter().enumerate() {
            if let Some(root) = self.roots.get_mut(index) {
                root.update(branch, &self.delegate);
            } else {
                let root = Branch::new(branch, &self.delegate);
                main.addItem(&root.item);
                self.roots.push(root);
            }
        }
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        self.delegate.ivars().live.set(false);
        self.delegate.cancel_wake();
        if let Some(main) = &self.main {
            for root in &self.roots {
                main.removeItem(&root.item);
            }
        }
        // Drop branches while their non-retained delegate is still alive.
        self.roots.clear();
    }
}
