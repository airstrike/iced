use std::sync::Arc;

use crate::Proxy;
use crate::core::{menu, window};

use super::platform;

pub struct Manager<Message: 'static> {
    proxy: Proxy<Message>,
    focus: crate::window::Focus,
    #[cfg(target_os = "macos")]
    native: platform::Menu,
}

impl<Message: 'static> Manager<Message> {
    pub fn new(proxy: Proxy<Message>, focus: crate::window::Focus) -> Self {
        Self {
            #[cfg(target_os = "macos")]
            native: platform::Menu::new(proxy.clone()),
            proxy,
            focus,
        }
    }

    pub fn refresh(
        &mut self,
        bar: menu::Bar<Message>,
        window: Option<(window::Id, &Arc<winit::window::Window>)>,
    ) {
        #[cfg(target_os = "macos")]
        self.native.update(super::snapshot::Snapshot::from_bar(
            bar,
            window,
            &self.proxy,
            self.focus.clone(),
        ));

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (&self.proxy, &self.focus, window);
            platform::update(&bar);
        }
    }

    pub fn flush(&mut self) {
        #[cfg(target_os = "macos")]
        self.native.flush();
    }
}
