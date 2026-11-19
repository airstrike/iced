use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol};

use crate::menu::callback::Callback;

use super::delegate::Delegate;

pub(super) struct Activation {
    pub(super) callback: Rc<Callback>,
    delegate: Retained<Delegate>,
}

// SAFETY: This main-thread-only NSObject subclass initializes its ivars before
// NSObject's initializer runs; its methods match the declared AppKit signatures.
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IcedApplicationMenuTarget"]
    #[ivars = Activation]
    pub(super) struct Target;

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

impl Target {
    pub(super) fn new(
        mtm: MainThreadMarker,
        callback: Rc<Callback>,
        delegate: &Delegate,
    ) -> Retained<Self> {
        let target = Self::alloc(mtm).set_ivars(Activation {
            callback,
            delegate: delegate.retain(),
        });
        // SAFETY: NSObject's initializer accepts our initialized subclass.
        unsafe { msg_send![super(target), init] }
    }
}
