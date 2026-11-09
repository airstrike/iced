use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSMenu, NSMenuDelegate};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSRunLoop,
};

use crate::Proxy;
use crate::menu::tracking::Tracking;
use crate::runtime::Action;

pub(super) struct Observer {
    pub(super) tracking: Tracking,
    pub(super) live: Cell<bool>,
    wake_scheduled: Cell<bool>,
    wake: Box<dyn Fn()>,
}

// SAFETY: This main-thread-only NSObject subclass initializes its ivars before
// NSObject's initializer runs; its methods match the declared AppKit signatures.
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IcedApplicationMenuDelegate"]
    #[ivars = Observer]
    pub(super) struct Delegate;

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

impl Delegate {
    pub(super) fn new<Message: 'static>(
        mtm: MainThreadMarker,
        proxy: Proxy<Message>,
    ) -> Retained<Self> {
        let delegate = Self::alloc(mtm).set_ivars(Observer {
            tracking: Tracking::default(),
            live: Cell::new(true),
            wake_scheduled: Cell::new(false),
            wake: Box::new(move || proxy.send_action(Action::Tick)),
        });
        // SAFETY: NSObject's initializer accepts our initialized subclass.
        unsafe { msg_send![super(delegate), init] }
    }

    pub(super) fn schedule_wake(&self) {
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

    pub(super) fn cancel_wake(&self) {
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
