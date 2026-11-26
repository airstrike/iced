//! Access the clipboard.
use crate::core::clipboard::{Content, Error, Kind};

#[cfg(any(target_arch = "wasm32", test))]
mod capture;
#[cfg(target_arch = "wasm32")]
mod owner;
#[cfg(any(target_arch = "wasm32", test))]
mod paste;
#[cfg(any(target_arch = "wasm32", test))]
mod target;

#[cfg(target_arch = "wasm32")]
pub(crate) use owner::Owner;

pub use platform::*;

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::*;

    use std::sync::{Arc, Mutex};
    use std::thread;

    /// A buffer for short-term storage and transfer within and between
    /// applications.
    pub struct Clipboard {
        state: State,
    }

    enum State {
        Connected {
            clipboard: Arc<Mutex<arboard::Clipboard>>,
        },
        Unavailable,
    }

    impl Clipboard {
        /// Creates a new [`Clipboard`] for the given window.
        pub fn new() -> Self {
            let clipboard = arboard::Clipboard::new();

            let state = match clipboard {
                Ok(clipboard) => State::Connected {
                    clipboard: Arc::new(Mutex::new(clipboard)),
                },
                Err(_) => State::Unavailable,
            };

            Clipboard { state }
        }

        /// Reads the current content of the [`Clipboard`] as text.
        pub fn read(
            &self,
            kind: Kind,
            callback: impl FnOnce(Result<Content, Error>) + Send + 'static,
        ) {
            let State::Connected { clipboard } = &self.state else {
                callback(Err(Error::ClipboardUnavailable));
                return;
            };

            let clipboard = clipboard.clone();

            let _ = thread::spawn(move || {
                let Ok(mut clipboard) = clipboard.lock() else {
                    callback(Err(Error::ClipboardUnavailable));
                    return;
                };

                let get = clipboard.get();

                let result = match kind {
                    Kind::Text => get.text().map(Content::Text),
                    Kind::Html => get.html().map(Content::Html),
                    #[cfg(feature = "image")]
                    Kind::Image => get.image().map(|image| {
                        let rgba = crate::core::Bytes::from_owner(image.bytes);
                        let size = crate::core::Size {
                            width: image.width as u32,
                            height: image.height as u32,
                        };

                        Content::Image(crate::core::clipboard::Image { rgba, size })
                    }),
                    Kind::Files => get.file_list().map(Content::Files),
                    #[cfg(all(
                        unix,
                        not(any(target_os = "macos", target_os = "android", target_os = "ios"))
                    ))]
                    Kind::Primary => {
                        use arboard::{GetExtLinux, LinuxClipboardKind};

                        get.clipboard(LinuxClipboardKind::Primary)
                            .text()
                            .map(Content::Text)
                    }
                    kind => {
                        log::warn!("unsupported clipboard kind: {kind:?}");

                        Err(arboard::Error::ContentNotAvailable)
                    }
                }
                .map_err(to_error);

                callback(result);
            });
        }

        /// Writes the given text contents to the [`Clipboard`].
        pub fn write(
            &mut self,
            content: Content,
            callback: impl FnOnce(Result<(), Error>) + Send + 'static,
        ) {
            let State::Connected { clipboard } = &self.state else {
                callback(Err(Error::ClipboardUnavailable));
                return;
            };

            let clipboard = clipboard.clone();

            let _ = thread::spawn(move || {
                let Ok(mut clipboard) = clipboard.lock() else {
                    callback(Err(Error::ClipboardUnavailable));
                    return;
                };

                let set = clipboard.set();

                let result = match content {
                    Content::Text(text) => set.text(text),
                    Content::Html(html) => set.html(html, None),
                    Content::Rich { html, text } => set.html(html, Some(text)),
                    #[cfg(feature = "image")]
                    Content::Image(image) => set.image(arboard::ImageData {
                        bytes: image.rgba.as_ref().into(),
                        width: image.size.width as usize,
                        height: image.size.height as usize,
                    }),
                    Content::Files(files) => set.file_list(&files),
                    content => {
                        log::warn!("unsupported clipboard content: {content:?}");

                        Err(arboard::Error::ClipboardNotSupported)
                    }
                }
                .map_err(to_error);

                callback(result);
            });
        }
    }

    impl Clipboard {
        /// Makes `text` the primary selection, where the system has one.
        pub fn write_primary(&mut self, text: String) {
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "android", target_os = "ios"))
            ))]
            {
                use arboard::{LinuxClipboardKind, SetExtLinux};

                let State::Connected { clipboard } = &self.state else {
                    return;
                };
                let clipboard = clipboard.clone();

                let _ = thread::spawn(move || {
                    if let Ok(mut clipboard) = clipboard.lock() {
                        let _ = clipboard
                            .set()
                            .clipboard(LinuxClipboardKind::Primary)
                            .text(text);
                    }
                });
            }

            #[cfg(not(all(
                unix,
                not(any(target_os = "macos", target_os = "android", target_os = "ios"))
            )))]
            let _ = text;
        }
    }

    fn to_error(error: arboard::Error) -> Error {
        match error {
            arboard::Error::ContentNotAvailable => Error::ContentNotAvailable,
            arboard::Error::ClipboardNotSupported => Error::ClipboardUnavailable,
            arboard::Error::ClipboardOccupied => Error::ClipboardOccupied,
            arboard::Error::ConversionFailure => Error::ConversionFailure,
            arboard::Error::Unknown { description } => Error::Unknown {
                description: Arc::new(description),
            },
            error => Error::Unknown {
                description: Arc::new(error.to_string()),
            },
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod rich_tests;

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;

    use super::capture::{self, Capture, Sink};
    use super::owner::{Owner, Registry};
    use super::paste::{self, Outcome, Paste, Read};
    use super::target::Target;

    use crate::core::window;

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use wasm_bindgen_futures::JsFuture;
    use web_sys::js_sys;
    use web_sys::wasm_bindgen::closure::Closure;
    use web_sys::wasm_bindgen::{JsCast, JsValue};

    /// A buffer for short-term storage and transfer within and between
    /// applications.
    ///
    /// Reading through `navigator.clipboard` asks the user for permission in
    /// most browsers, so a paste shortcut is fulfilled by the `paste` event
    /// the browser fires for it instead, which carries the text for free.
    ///
    /// The browser's Edit menu reaches the focused widget through a Copy, Cut
    /// or Paste key press sent to the canvas while its clipboard event runs.
    pub struct Clipboard {
        state: State,
        paste: Rc<RefCell<Paste>>,
        sink: Rc<RefCell<Sink>>,
        owners: Rc<RefCell<Registry>>,
        listeners: Vec<Listener>,
    }

    enum State {
        Connected { clipboard: web_sys::Clipboard },
        Unavailable,
    }

    struct Listener {
        kind: &'static str,
        closure: Closure<dyn FnMut(web_sys::Event)>,
    }

    /// An editing command of the browser's Edit menu, sent to the canvas as a
    /// key press that winit turns into an iced turn on the same stack.
    #[derive(Debug, Clone, Copy)]
    enum Command {
        Copy,
        Cut,
        Paste,
    }

    impl Clipboard {
        /// Creates a new [`Clipboard`] for the given window.
        pub fn new() -> Self {
            let window = web_sys::window();

            // `navigator.clipboard` is only present in secure contexts
            // (HTTPS or localhost).
            let clipboard = window
                .as_ref()
                .map(|window| window.navigator().clipboard())
                .filter(|clipboard| !clipboard.is_undefined());

            let state = match clipboard {
                Some(clipboard) => State::Connected { clipboard },
                None => State::Unavailable,
            };

            let paste = Rc::new(RefCell::new(Paste::default()));
            let sink = Rc::new(RefCell::new(Sink::default()));
            let owners = Rc::new(RefCell::new(Registry::default()));
            let mut listeners = Vec::new();

            if let Some(window) = window {
                listeners.push(Listener::new(&window, "keydown", {
                    let paste = paste.clone();
                    let owners = owners.clone();
                    let clipboard = state.clipboard();

                    move |event| on_key_down(event, &paste, &owners, clipboard.as_ref())
                }));

                listeners.push(Listener::new(&window, "pointerdown", {
                    let paste = paste.clone();
                    let clipboard = state.clipboard();

                    move |_| {
                        let reads = paste.borrow_mut().interrupt();
                        fall_back(reads, clipboard.as_ref());
                    }
                }));

                listeners.push(Listener::new(&window, "paste", {
                    let paste = paste.clone();
                    let owners = owners.clone();

                    move |event| on_paste(event, &paste, &owners)
                }));

                for (kind, command) in [("copy", Command::Copy), ("cut", Command::Cut)] {
                    listeners.push(Listener::new(&window, kind, {
                        let sink = sink.clone();
                        let owners = owners.clone();

                        move |event| on_copy(event, command, &sink, &owners)
                    }));
                }

                // Safari greys out Cut and Copy without a DOM selection, unless
                // these events are cancelled. Firefox fires none of them.
                for kind in ["beforecopy", "beforecut", "beforepaste"] {
                    listeners.push(Listener::new(&window, kind, {
                        let owners = owners.clone();

                        move |event| {
                            if owners.borrow().target(&event).into_canvas().is_some() {
                                event.prevent_default();
                            }
                        }
                    }));
                }
            }

            Clipboard {
                state,
                paste,
                sink,
                owners,
                listeners,
            }
        }

        /// Reads the current content of the [`Clipboard`] as text.
        pub fn read(&self, kind: Kind, callback: impl FnOnce(Result<Content, Error>) + 'static) {
            match kind {
                Kind::Text => {
                    let text = self.paste.borrow_mut().take_text();

                    if let Some(text) = text {
                        callback(Ok(Content::Text(text)));
                        return;
                    }

                    let read = self.paste.borrow_mut().wait(Box::new(callback));

                    if let Some(read) = read {
                        read_text(self.state.clipboard().as_ref(), read);
                    }
                }
                kind => {
                    log::warn!("unsupported clipboard kind: {kind:?}");

                    callback(Err(Error::ContentNotAvailable));
                }
            }
        }

        /// The web has no primary selection: does nothing.
        pub fn write_primary(&mut self, _text: String) {}

        /// Writes the given text contents to the [`Clipboard`]. Of
        /// [`Content::Rich`], only the plain text is written.
        ///
        /// During the `copy` or `cut` event of the browser's Edit menu, the
        /// text goes to the event instead.
        pub fn write(
            &mut self,
            content: Content,
            callback: impl FnOnce(Result<(), Error>) + 'static,
        ) {
            let text = match content {
                Content::Text(text) | Content::Rich { text, .. } => text,
                content => {
                    log::warn!("unsupported clipboard content: {content:?}");

                    callback(Err(Error::ClipboardUnavailable));
                    return;
                }
            };

            let write = self.sink.borrow_mut().write(text);

            let text = match write {
                capture::Write::Captured => {
                    callback(Ok(()));
                    return;
                }
                capture::Write::Passed(text) => text,
            };

            let State::Connected { clipboard } = &self.state else {
                callback(Err(Error::ClipboardUnavailable));
                return;
            };

            let write = JsFuture::from(clipboard.write_text(&text));

            wasm_bindgen_futures::spawn_local(async move {
                let result = write.await.map(|_| ()).map_err(to_error);

                callback(result);
            });
        }

        /// Answers the clipboard events of the elements of the window `id`.
        pub(crate) fn own(&mut self, id: window::Id, owner: Owner) {
            self.owners.borrow_mut().insert(id, owner);
        }

        /// Leaves the clipboard events of the closed window `id` to the page.
        pub(crate) fn disown(&mut self, id: window::Id) {
            self.owners.borrow_mut().remove(id);
        }
    }

    impl Drop for Clipboard {
        fn drop(&mut self) {
            let Some(window) = web_sys::window() else {
                return;
            };

            for listener in &self.listeners {
                let _ = window.remove_event_listener_with_callback_and_bool(
                    listener.kind,
                    listener.closure.as_ref().unchecked_ref(),
                    true,
                );
            }
        }
    }

    impl State {
        fn clipboard(&self) -> Option<web_sys::Clipboard> {
            match self {
                State::Connected { clipboard } => Some(clipboard.clone()),
                State::Unavailable => None,
            }
        }
    }

    impl Listener {
        /// Listens to `kind` events on the `window` in the capture phase, so the
        /// listener runs before the canvas listeners of winit.
        fn new(
            window: &web_sys::Window,
            kind: &'static str,
            listener: impl FnMut(web_sys::Event) + 'static,
        ) -> Self {
            let closure = Closure::<dyn FnMut(web_sys::Event)>::new(listener);

            let _ = window.add_event_listener_with_callback_and_bool(
                kind,
                closure.as_ref().unchecked_ref(),
                true,
            );

            Self { kind, closure }
        }
    }

    impl Command {
        /// The key whose press winit turns into this command.
        fn key(self) -> &'static str {
            match self {
                Command::Copy => "Copy",
                Command::Cut => "Cut",
                Command::Paste => "Paste",
            }
        }

        /// Sends the key press of the command to `canvas`. winit runs the iced
        /// turn for it before this returns, unless its loop is busy.
        fn send(self, canvas: &web_sys::HtmlCanvasElement) {
            let init = web_sys::KeyboardEventInit::new();
            init.set_bubbles(true);
            init.set_cancelable(true);
            init.set_key(self.key());

            if let Ok(press) =
                web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init)
            {
                let _ = canvas.dispatch_event(&press);
            }
        }
    }

    fn on_key_down(
        event: web_sys::Event,
        paste: &Rc<RefCell<Paste>>,
        owners: &Rc<RefCell<Registry>>,
        clipboard: Option<&web_sys::Clipboard>,
    ) {
        // The key presses sent to winit below come back through here.
        if !event.is_trusted() {
            return;
        }

        let reads = paste.borrow_mut().interrupt();
        fall_back(reads, clipboard);

        let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };

        if !is_paste_shortcut(key) {
            return;
        }

        let shortcut = paste.borrow_mut().press();
        schedule_timeout(shortcut, paste, clipboard);

        // An element over the canvas (an input method's text field) forwards
        // its key presses to winit and lets the browser paste on its own.
        let Target::Canvas(canvas) = owners.borrow().target(&event) else {
            return;
        };

        // winit cancels every key press on its canvas, which would cancel the
        // browser's paste as well. Hand winit a copy of the key press instead,
        // and let the original one paste.
        event.stop_propagation();

        let init = web_sys::KeyboardEventInit::new();
        init.set_bubbles(true);
        init.set_cancelable(true);
        init.set_key(&key.key());
        init.set_code(&key.code());
        init.set_location(key.location());
        init.set_repeat(key.repeat());
        init.set_alt_key(key.alt_key());
        init.set_ctrl_key(key.ctrl_key());
        init.set_meta_key(key.meta_key());
        init.set_shift_key(key.shift_key());

        if let Ok(copy) =
            web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init)
        {
            let _ = canvas.dispatch_event(&copy);
        }
    }

    fn schedule_timeout(
        shortcut: u64,
        paste: &Rc<RefCell<Paste>>,
        clipboard: Option<&web_sys::Clipboard>,
    ) {
        let timeout = Closure::once_into_js({
            let paste = paste.clone();
            let clipboard = clipboard.cloned();

            move || {
                let reads = paste.borrow_mut().time_out(shortcut);
                fall_back(reads, clipboard.as_ref());
            }
        });

        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.unchecked_ref(),
                paste::TIMEOUT,
            );
        }
    }

    /// Reads the clipboard for the reads of a paste shortcut whose `paste`
    /// event did not come.
    fn fall_back(reads: Vec<Read>, clipboard: Option<&web_sys::Clipboard>) {
        for read in reads {
            read_text(clipboard, read);
        }
    }

    fn on_paste(event: web_sys::Event, paste: &Rc<RefCell<Paste>>, owners: &Rc<RefCell<Registry>>) {
        let Some(clipboard_event) = event.dyn_ref::<web_sys::ClipboardEvent>() else {
            return;
        };

        let target = owners.borrow().target(&event);

        let text = clipboard_event
            .clipboard_data()
            .and_then(|data| data.get_data("text/plain").ok());

        // A paste into an element of the page belongs to it, unless iced read
        // the clipboard for the key press that pasted.
        let outcome = paste.borrow_mut().paste(text.as_deref(), target);

        match outcome {
            Outcome::Element => {}
            Outcome::Shortcut(reads) => {
                event.prevent_default();

                for read in reads {
                    read(
                        text.clone()
                            .map(Content::Text)
                            .ok_or(Error::ContentNotAvailable),
                    );
                }
            }
            Outcome::Menu(canvas) => {
                event.prevent_default();

                Command::Paste.send(&canvas);
            }
        }
    }

    /// Answers the `copy` or `cut` event of the browser's Edit menu with the
    /// focused widget's own copy or cut.
    fn on_copy(
        event: web_sys::Event,
        command: Command,
        sink: &Rc<RefCell<Sink>>,
        owners: &Rc<RefCell<Registry>>,
    ) {
        let Some(clipboard_event) = event.dyn_ref::<web_sys::ClipboardEvent>() else {
            return;
        };

        let Some(canvas) = owners.borrow().target(&event).into_canvas() else {
            return;
        };

        let Some(data) = clipboard_event.clipboard_data() else {
            return;
        };

        let capture = match Capture::open(sink) {
            Ok(capture) => capture,
            Err(error) => {
                log::warn!("{error}");
                return;
            }
        };

        command.send(&canvas);

        // Nothing captured leaves the browser's own copy, of an empty
        // selection, in place.
        if let Some(captured) = capture.close() {
            captured.deliver(&event, &data);
        }
    }

    /// Whether the browser pastes on this key press: `Ctrl+V` or `Cmd+V`, and
    /// `Shift+Insert`.
    fn is_paste_shortcut(key: &web_sys::KeyboardEvent) -> bool {
        let is_v = key.key().eq_ignore_ascii_case("v") || key.code() == "KeyV";

        (is_v && (key.ctrl_key() || key.meta_key()) && !key.alt_key())
            || (key.key() == "Insert" && key.shift_key() && !key.ctrl_key() && !key.meta_key())
    }

    fn read_text(clipboard: Option<&web_sys::Clipboard>, callback: Read) {
        let Some(clipboard) = clipboard else {
            callback(Err(Error::ClipboardUnavailable));
            return;
        };

        let read = JsFuture::from(clipboard.read_text());

        wasm_bindgen_futures::spawn_local(async move {
            let result = read
                .await
                .map(|text| Content::Text(text.as_string().unwrap_or_default()))
                .map_err(to_error);

            callback(result);
        });
    }

    fn to_error(error: JsValue) -> Error {
        // Clipboard rejections surface as `DOMException` (e.g.
        // `NotAllowedError` when the user denies a permission prompt).
        let description = error
            .dyn_ref::<js_sys::Error>()
            .map(|error| String::from(error.message()))
            .unwrap_or_else(|| format!("{error:?}"));

        Error::Unknown {
            description: Arc::new(description),
        }
    }
}
