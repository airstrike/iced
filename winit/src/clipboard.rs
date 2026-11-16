//! Access the clipboard.
use crate::core::clipboard::{Content, Error, Kind};

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

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;

    use std::cell::RefCell;
    use std::mem;
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
    pub struct Clipboard {
        state: State,
        paste: Rc<RefCell<Paste>>,
        listeners: Vec<Listener>,
    }

    enum State {
        Connected { clipboard: web_sys::Clipboard },
        Unavailable,
    }

    /// A paste shortcut in flight.
    #[derive(Default)]
    struct Paste {
        /// A paste shortcut was pressed and its `paste` event has not fired yet.
        is_expected: bool,
        /// The text of a `paste` event that no read has taken yet.
        text: Option<String>,
        /// The reads waiting for the expected `paste` event.
        reads: Vec<Box<dyn FnOnce(Result<Content, Error>)>>,
    }

    struct Listener {
        kind: &'static str,
        closure: Closure<dyn FnMut(web_sys::Event)>,
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
            let mut listeners = Vec::new();

            if let Some(window) = window {
                listeners.push(Listener::new(&window, "keydown", {
                    let paste = paste.clone();
                    let clipboard = state.clipboard();

                    move |event| on_key_down(event, &paste, clipboard.as_ref())
                }));

                listeners.push(Listener::new(&window, "pointerdown", {
                    let paste = paste.clone();

                    move |_| paste.borrow_mut().text = None
                }));

                listeners.push(Listener::new(&window, "paste", {
                    let paste = paste.clone();

                    move |event| on_paste(event, &paste)
                }));
            }

            Clipboard {
                state,
                paste,
                listeners,
            }
        }

        /// Reads the current content of the [`Clipboard`] as text.
        pub fn read(&self, kind: Kind, callback: impl FnOnce(Result<Content, Error>) + 'static) {
            match kind {
                Kind::Text => {
                    let mut paste = self.paste.borrow_mut();

                    if let Some(text) = paste.text.take() {
                        drop(paste);
                        callback(Ok(Content::Text(text)));
                    } else if paste.is_expected {
                        paste.reads.push(Box::new(callback));
                    } else {
                        drop(paste);
                        read_text(self.state.clipboard().as_ref(), callback);
                    }
                }
                kind => {
                    log::warn!("unsupported clipboard kind: {kind:?}");

                    callback(Err(Error::ContentNotAvailable));
                }
            }
        }

        /// Writes the given text contents to the [`Clipboard`].
        pub fn write(
            &mut self,
            content: Content,
            callback: impl FnOnce(Result<(), Error>) + 'static,
        ) {
            let State::Connected { clipboard } = &self.state else {
                callback(Err(Error::ClipboardUnavailable));
                return;
            };

            match content {
                Content::Text(text) => {
                    let write = JsFuture::from(clipboard.write_text(&text));

                    wasm_bindgen_futures::spawn_local(async move {
                        let result = write.await.map(|_| ()).map_err(to_error);

                        callback(result);
                    });
                }
                content => {
                    log::warn!("unsupported clipboard content: {content:?}");

                    callback(Err(Error::ClipboardUnavailable));
                }
            }
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

    fn on_key_down(
        event: web_sys::Event,
        paste: &Rc<RefCell<Paste>>,
        clipboard: Option<&web_sys::Clipboard>,
    ) {
        // The copy handed to winit below comes back through here.
        if !event.is_trusted() {
            return;
        }

        paste.borrow_mut().text = None;

        let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };

        if !is_paste_shortcut(key) {
            return;
        }

        paste.borrow_mut().is_expected = true;
        schedule_fallback(paste, clipboard);

        // An element over the canvas (an input method's text field) forwards
        // its key presses to winit and lets the browser paste on its own.
        let Some(canvas) = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
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

    fn schedule_fallback(paste: &Rc<RefCell<Paste>>, clipboard: Option<&web_sys::Clipboard>) {
        // The browser fires `paste` right after the key press, before any
        // timer. Without one, fall back to reading the clipboard.
        let fallback = Closure::once_into_js({
            let paste = paste.clone();
            let clipboard = clipboard.cloned();

            move || {
                let reads = {
                    let mut paste = paste.borrow_mut();

                    if !paste.is_expected {
                        return;
                    }

                    paste.is_expected = false;
                    mem::take(&mut paste.reads)
                };

                for callback in reads {
                    read_text(clipboard.as_ref(), callback);
                }
            }
        });

        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback(fallback.unchecked_ref());
        }
    }

    fn on_paste(event: web_sys::Event, paste: &Rc<RefCell<Paste>>) {
        let Some(clipboard_event) = event.dyn_ref::<web_sys::ClipboardEvent>() else {
            return;
        };

        let is_canvas = event
            .target()
            .is_some_and(|target| target.has_type::<web_sys::HtmlCanvasElement>());

        // A paste into an element of the page belongs to it, unless iced read
        // the clipboard for the key press that pasted.
        if !is_canvas && paste.borrow().reads.is_empty() {
            return;
        }

        event.prevent_default();

        let text = clipboard_event
            .clipboard_data()
            .and_then(|data| data.get_data("text/plain").ok());

        let reads = {
            let mut paste = paste.borrow_mut();
            paste.is_expected = false;

            let reads = mem::take(&mut paste.reads);

            if reads.is_empty() {
                paste.text = text.clone();
            }

            reads
        };

        for callback in reads {
            callback(
                text.clone()
                    .map(Content::Text)
                    .ok_or(Error::ContentNotAvailable),
            );
        }
    }

    /// Whether the browser pastes on this key press: `Ctrl+V` or `Cmd+V`, and
    /// `Shift+Insert`.
    fn is_paste_shortcut(key: &web_sys::KeyboardEvent) -> bool {
        let is_v = key.key().eq_ignore_ascii_case("v") || key.code() == "KeyV";

        (is_v && (key.ctrl_key() || key.meta_key()) && !key.alt_key())
            || (key.key() == "Insert" && key.shift_key() && !key.ctrl_key() && !key.meta_key())
    }

    fn read_text(
        clipboard: Option<&web_sys::Clipboard>,
        callback: impl FnOnce(Result<Content, Error>) + 'static,
    ) {
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
