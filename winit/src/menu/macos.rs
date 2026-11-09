use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuDelegate, NSMenuItem};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSRunLoop,
    NSString, ns_string,
};

use crate::Proxy;
use crate::core::{keyboard, menu};
use crate::runtime::Action;

use super::snapshot::{self, Callback, Snapshot};
use super::tracking::{Mode, Tracking};

/// Owns only the roots appended to winit's existing application menu.
pub struct Menu {
    main: Option<Retained<NSMenu>>,
    roots: Vec<Branch>,
    delegate: Retained<Delegate>,
    pending: Option<Snapshot>,
}

struct Branch {
    item: Retained<NSMenuItem>,
    menu: Retained<NSMenu>,
    entries: Vec<Entry>,
}

enum Entry {
    Item(Item),
    Submenu(Branch),
    Separator(Retained<NSMenuItem>),
}

struct Item {
    item: Retained<NSMenuItem>,
    target: Option<Retained<Target>>,
}

struct Activation {
    callback: Rc<Callback>,
    delegate: Retained<Delegate>,
}

struct Observer {
    tracking: Tracking,
    live: Cell<bool>,
    wake_scheduled: Cell<bool>,
    wake: Box<dyn Fn()>,
}

// SAFETY: Both classes are main-thread-only NSObject subclasses. Their methods
// match AppKit's target/action and NSMenuDelegate signatures, and their ivars
// are initialized before NSObject's initializer runs.
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IcedApplicationMenuTarget"]
    #[ivars = Activation]
    struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        #[unsafe(method(activate:))]
        fn activate(&self, _sender: &AnyObject) {
            // User factories may run nested default-mode loops. Keep both the
            // executing target and its presented snapshot alive until return.
            let target = self.retain();
            let activation = target.ivars();
            activation.delegate.ivars().tracking.begin();
            activation.callback.invoke();
            activation.delegate.ivars().tracking.end();
            activation.delegate.schedule_wake();
        }
    }
);

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IcedApplicationMenuDelegate"]
    #[ivars = Observer]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    impl Delegate {
        #[unsafe(method(resumeMenuUpdates:))]
        fn resume_menu_updates(&self, _sender: Option<&AnyObject>) {
            self.ivars().wake_scheduled.set(false);
            (self.ivars().wake)();
        }
    }

    unsafe impl NSMenuDelegate for Delegate {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) {
            self.ivars().tracking.begin();
        }

        #[unsafe(method(menuDidClose:))]
        fn menu_did_close(&self, _menu: &NSMenu) {
            self.ivars().tracking.end();
            // Cancellation produces no action. Schedule the wake in default
            // mode so a nested loop cannot consume it before selection unwinds.
            self.schedule_wake();
        }
    }
);

impl Target {
    fn new(mtm: MainThreadMarker, callback: Rc<Callback>, delegate: &Delegate) -> Retained<Self> {
        let target = Self::alloc(mtm).set_ivars(Activation {
            callback,
            delegate: delegate.retain(),
        });
        // SAFETY: NSObject's initializer accepts our initialized subclass.
        unsafe { msg_send![super(target), init] }
    }
}

impl Delegate {
    fn new<Message: 'static>(mtm: MainThreadMarker, proxy: Proxy<Message>) -> Retained<Self> {
        let delegate = Self::alloc(mtm).set_ivars(Observer {
            tracking: Tracking::default(),
            live: Cell::new(true),
            wake_scheduled: Cell::new(false),
            wake: Box::new(move || proxy.send_action(Action::Tick)),
        });
        // SAFETY: NSObject's initializer accepts our initialized subclass.
        unsafe { msg_send![super(delegate), init] }
    }

    fn schedule_wake(&self) {
        if !self.ivars().live.get() || self.ivars().wake_scheduled.replace(true) {
            return;
        }
        // SAFETY: Foundation exports this immutable process-lifetime NSString.
        let modes = NSArray::from_slice(&[unsafe { NSDefaultRunLoopMode }]);
        // SAFETY: The selector is defined above with an optional object argument.
        // This main-thread run loop retains the delegate until delivery; Menu's
        // destructor cancels the request before releasing its scoped ownership.
        unsafe {
            NSRunLoop::currentRunLoop().performSelector_target_argument_order_modes(
                sel!(resumeMenuUpdates:),
                self,
                None,
                0,
                &modes,
            );
        }
    }

    fn cancel_wake(&self) {
        // SAFETY: This is the same selector, target and argument used when
        // scheduling. Cancellation releases any run-loop-owned target reference.
        unsafe {
            NSRunLoop::currentRunLoop().cancelPerformSelector_target_argument(
                sel!(resumeMenuUpdates:),
                self,
                None,
            );
        }
        self.ivars().wake_scheduled.set(false);
    }
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
        if self.delegate.ivars().tracking.is_frozen(run_loop_mode()) {
            self.pending = Some(snapshot);
            self.delegate.schedule_wake();
        } else {
            self.pending = None;
            self.apply(snapshot);
        }
    }

    pub fn flush(&mut self) {
        let mode = run_loop_mode();
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
                collect_shortcuts(&item, &mut shortcuts);
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

impl Branch {
    fn new(snapshot: snapshot::Branch, delegate: &Delegate) -> Self {
        let mtm = MainThreadMarker::from(delegate);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(delegate)));
        let item = NSMenuItem::new(mtm);
        item.setSubmenu(Some(&menu));
        let mut branch = Self {
            item,
            menu,
            entries: Vec::new(),
        };
        branch.update(snapshot, delegate);
        branch
    }

    fn update(&mut self, snapshot: snapshot::Branch, delegate: &Delegate) {
        let title = NSString::from_str(&snapshot.title);
        self.item.setTitle(&title);
        self.item.setEnabled(snapshot.enabled);
        self.menu.setTitle(&title);

        let same_shape = self.entries.len() == snapshot.entries.len()
            && self
                .entries
                .iter()
                .zip(&snapshot.entries)
                .all(|(entry, snapshot)| {
                    matches!(
                        (entry, snapshot),
                        (Entry::Item(_), snapshot::Entry::Item(_))
                            | (Entry::Submenu(_), snapshot::Entry::Submenu(_))
                            | (Entry::Separator(_), snapshot::Entry::Separator)
                    )
                });

        if same_shape {
            for (entry, snapshot) in self.entries.iter_mut().zip(snapshot.entries) {
                match (entry, snapshot) {
                    (Entry::Item(item), snapshot::Entry::Item(snapshot)) => {
                        item.update(snapshot, delegate);
                    }
                    (Entry::Submenu(branch), snapshot::Entry::Submenu(snapshot)) => {
                        branch.update(snapshot, delegate);
                    }
                    _ => {}
                }
            }
        } else {
            for entry in self.entries.drain(..) {
                self.menu.removeItem(entry.item());
            }
            for entry in snapshot.entries {
                let entry = Entry::new(entry, delegate);
                self.menu.addItem(entry.item());
                self.entries.push(entry);
            }
        }
    }
}

impl Drop for Branch {
    fn drop(&mut self) {
        self.menu.setDelegate(None);
    }
}

impl Entry {
    fn new(snapshot: snapshot::Entry, delegate: &Delegate) -> Self {
        let mtm = MainThreadMarker::from(delegate);
        match snapshot {
            snapshot::Entry::Item(snapshot) => {
                let mut item = Item {
                    item: NSMenuItem::new(mtm),
                    target: None,
                };
                // SAFETY: Every target installed by Item::update implements this
                // exact selector, and Item retains it until the target is cleared.
                unsafe {
                    item.item.setAction(Some(sel!(activate:)));
                }
                item.update(snapshot, delegate);
                Self::Item(item)
            }
            snapshot::Entry::Submenu(snapshot) => Self::Submenu(Branch::new(snapshot, delegate)),
            snapshot::Entry::Separator => Self::Separator(NSMenuItem::separatorItem(mtm)),
        }
    }

    fn item(&self) -> &NSMenuItem {
        match self {
            Self::Item(item) => &item.item,
            Self::Submenu(branch) => &branch.item,
            Self::Separator(item) => item,
        }
    }
}

impl Item {
    fn update(&mut self, snapshot: snapshot::Item, delegate: &Delegate) {
        self.item.setTitle(&NSString::from_str(&snapshot.title));
        self.item.setEnabled(snapshot.enabled);
        self.item.setState(if snapshot.checked { 1 } else { 0 });

        if let Some(shortcut) = snapshot.shortcut {
            self.item
                .setKeyEquivalent(&NSString::from_str(&shortcut.key().to_string()));
            self.item
                .setKeyEquivalentModifierMask(modifiers(shortcut.modifiers()));
        } else {
            self.item.setKeyEquivalent(ns_string!(""));
        }

        self.clear_target();
        self.target = snapshot.callback.map(|callback| {
            let target = Target::new(MainThreadMarker::from(&*self.item), callback, delegate);
            // SAFETY: The selector is Target::activate:, and self.target retains
            // this object until clear_target disconnects AppKit's weak pointer.
            unsafe {
                self.item.setTarget(Some(&target));
            }
            target
        });
    }

    fn clear_target(&mut self) {
        // SAFETY: Clearing the non-retained target before releasing our strong
        // reference ensures retained native items cannot message freed storage.
        unsafe {
            self.item.setTarget(None);
        }
        if let Some(target) = self.target.take() {
            target.ivars().callback.retire();
        }
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        self.clear_target();
        // SAFETY: An externally retained item must not resolve activate: through
        // the responder chain after its scoped target has been removed.
        unsafe {
            self.item.setAction(None);
        }
    }
}

fn modifiers(modifiers: keyboard::Modifiers) -> NSEventModifierFlags {
    let mut flags = NSEventModifierFlags::empty();
    if modifiers.control() {
        flags |= NSEventModifierFlags::Control;
    }
    if modifiers.alt() {
        flags |= NSEventModifierFlags::Option;
    }
    if modifiers.logo() {
        flags |= NSEventModifierFlags::Command;
    }
    if modifiers.shift() {
        flags |= NSEventModifierFlags::Shift;
    }
    flags
}

fn collect_shortcuts(item: &NSMenuItem, shortcuts: &mut HashSet<menu::Shortcut>) {
    let key = item.keyEquivalent().to_string();
    let mut chars = key.chars();
    if let Some(key) = chars.next()
        && chars.next().is_none()
    {
        let flags = item.keyEquivalentModifierMask();
        let mut modifiers = keyboard::Modifiers::empty();
        if flags.contains(NSEventModifierFlags::Control) {
            modifiers |= keyboard::Modifiers::CTRL;
        }
        if flags.contains(NSEventModifierFlags::Option) {
            modifiers |= keyboard::Modifiers::ALT;
        }
        if flags.contains(NSEventModifierFlags::Command) {
            modifiers |= keyboard::Modifiers::LOGO;
        }
        if flags.contains(NSEventModifierFlags::Shift) {
            modifiers |= keyboard::Modifiers::SHIFT;
        }
        if let Ok(shortcut) = menu::Shortcut::new(modifiers, key) {
            let _ = shortcuts.insert(shortcut);
        }
    }
    if let Some(menu) = item.submenu() {
        for item in menu.itemArray() {
            collect_shortcuts(&item, shortcuts);
        }
    }
}

fn run_loop_mode() -> Mode {
    let Some(mode) = NSRunLoop::currentRunLoop().currentMode() else {
        return Mode::Other;
    };
    // SAFETY: Foundation exports this immutable process-lifetime NSString.
    if mode.isEqualToString(unsafe { NSDefaultRunLoopMode }) {
        Mode::Default
    } else {
        Mode::Other
    }
}
