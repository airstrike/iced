use objc2_foundation::{NSDefaultRunLoopMode, NSRunLoop};

use crate::menu::tracking::Mode;

pub(super) fn mode() -> Mode {
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
