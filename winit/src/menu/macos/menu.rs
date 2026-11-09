use std::collections::HashSet;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::MainThreadMarker;

use crate::Proxy;
use crate::core::menu::Role;
use crate::menu::snapshot::Snapshot;

use super::branch::Branch;
use super::delegate::Delegate;
use super::registration::{self, Registration};
use super::{run_loop, shortcut};

/// Owns only the roots appended to winit's existing application menu.
pub struct Menu {
    main: Option<Retained<NSMenu>>,
    roots: Vec<Branch>,
    delegate: Retained<Delegate>,
    pending: Option<Snapshot>,
    help: Registration,
}

impl Menu {
    pub fn new<Message: 'static>(proxy: Proxy<Message>) -> Self {
        let mtm = MainThreadMarker::new().expect("winit polls its instance on the main thread");
        Self {
            main: None,
            roots: Vec::new(),
            delegate: Delegate::new(mtm, proxy),
            pending: None,
            help: Registration::new(mtm),
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

        let mut window_assigned = false;
        let mut help_assigned = false;
        for (index, mut branch) in snapshot.roots.into_iter().enumerate() {
            let assigned = match branch.role {
                Some(Role::Window) => Some(&mut window_assigned),
                Some(Role::Help) => Some(&mut help_assigned),
                None => None,
            };
            if let Some(assigned) = assigned {
                if *assigned {
                    branch.role = None;
                }
                *assigned = true;
            }

            if let Some(root) = self.roots.get_mut(index) {
                if root.role == branch.role {
                    root.update(branch, &self.delegate);
                } else {
                    // AppKit cannot unmark a Window menu through setWindowsMenu(None).
                    let position = main.indexOfItem(&root.item);
                    main.removeItem(&root.item);
                    *root = Branch::new(branch, &self.delegate);
                    main.insertItem_atIndex(&root.item, position);
                    if root.role == Some(Role::Window) {
                        registration::window(&root.menu);
                    }
                }
            } else {
                let root = Branch::new(branch, &self.delegate);
                main.addItem(&root.item);
                if root.role == Some(Role::Window) {
                    registration::window(&root.menu);
                }
                self.roots.push(root);
            }
        }

        let help = self.roots.iter().find(|root| root.role == Some(Role::Help));
        self.help.update(help.map(|root| &*root.menu));
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        self.delegate.ivars().live.set(false);
        self.delegate.cancel_wake();
        self.help.update(None);
        if let Some(main) = &self.main {
            for root in &self.roots {
                main.removeItem(&root.item);
            }
        }
        // Drop branches while their non-retained delegate is still alive.
        self.roots.clear();
    }
}
