//! The `copy` and `cut` events of the browser's Edit menu, answered by an
//! iced turn on the same stack.
//!
//! A `copy` or `cut` listener must fill the event's data before it returns,
//! while iced widgets only answer during a turn of its loop. The listener opens
//! a [`Capture`] and sends the canvas a Copy or Cut key press, which winit runs
//! as a whole iced turn before the dispatch returns. The clipboard writes of
//! that turn land in the capture instead of `navigator.clipboard`.
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

/// Where the text written to the clipboard goes.
#[derive(Debug, Default)]
pub struct Sink {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    /// Writes go to the system clipboard.
    #[default]
    Closed,
    /// Writes go to the event of the open [`Capture`], the last one winning.
    Open { text: Option<String> },
}

/// What a write to the clipboard turned into.
#[derive(Debug, PartialEq, Eq)]
pub enum Write {
    /// An open [`Capture`] took the text for its event.
    Captured,
    /// No capture is open: the text goes to the system clipboard.
    Passed(String),
}

/// A `copy` or `cut` event whose iced turn is running: until it closes, the
/// clipboard's writes fill the event instead of the system clipboard.
#[must_use = "a capture delivers its text only once it is closed"]
#[derive(Debug)]
pub struct Capture {
    sink: Rc<RefCell<Sink>>,
}

/// The text a [`Capture`] took, which its event must carry.
#[must_use = "captured text reaches the clipboard only through its event"]
#[derive(Debug, PartialEq, Eq)]
pub struct Captured(String);

/// Why a [`Capture`] could not open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The event fired inside the iced turn of another capture.
    #[error("a clipboard capture is already open")]
    AlreadyOpen,
}

impl Sink {
    /// Writes `text` to the open capture, or passes it on to the system
    /// clipboard when none is open.
    pub fn write(&mut self, text: String) -> Write {
        match &mut self.state {
            State::Open { text: captured } => {
                *captured = Some(text);
                Write::Captured
            }
            State::Closed => Write::Passed(text),
        }
    }
}

impl Capture {
    /// Opens a capture over the writes to `sink`.
    pub fn open(sink: &Rc<RefCell<Sink>>) -> Result<Self, Error> {
        let mut inner = sink.borrow_mut();

        if let State::Open { .. } = inner.state {
            return Err(Error::AlreadyOpen);
        }

        inner.state = State::Open { text: None };

        Ok(Self { sink: sink.clone() })
    }

    /// Closes the capture. Returns the text written while it was open, if
    /// any; the event keeps its default otherwise.
    pub fn close(self) -> Option<Captured> {
        let state = mem::take(&mut self.sink.borrow_mut().state);

        match state {
            State::Open { text } => text.map(Captured),
            State::Closed => None,
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Ok(mut sink) = self.sink.try_borrow_mut() {
            sink.state = State::Closed;
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Captured {
    /// Puts the text in the event's `data` and cancels the event, so the
    /// browser copies it instead of the page's selection.
    ///
    /// The event keeps its default if the browser refuses the data.
    pub fn deliver(self, event: &web_sys::Event, data: &web_sys::DataTransfer) {
        match data.set_data("text/plain", &self.0) {
            Ok(()) => event.prevent_default(),
            Err(error) => log::warn!("the browser refused the copied text: {error:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink() -> Rc<RefCell<Sink>> {
        Rc::new(RefCell::new(Sink::default()))
    }

    #[test]
    fn a_write_without_capture_goes_to_the_system_clipboard() {
        let sink = sink();

        assert_eq!(
            sink.borrow_mut().write("text".to_owned()),
            Write::Passed("text".to_owned())
        );
    }

    #[test]
    fn a_write_during_a_capture_fills_its_event() {
        let sink = sink();
        let capture = Capture::open(&sink).unwrap();

        assert_eq!(sink.borrow_mut().write("text".to_owned()), Write::Captured);

        assert_eq!(capture.close(), Some(Captured("text".to_owned())));
    }

    #[test]
    fn a_capture_without_write_keeps_the_default() {
        let sink = sink();
        let capture = Capture::open(&sink).unwrap();

        assert_eq!(capture.close(), None);
    }

    #[test]
    fn a_write_after_the_capture_closes_goes_to_the_system_clipboard() {
        let sink = sink();
        let capture = Capture::open(&sink).unwrap();
        let _ = capture.close();

        assert_eq!(
            sink.borrow_mut().write("later".to_owned()),
            Write::Passed("later".to_owned())
        );
    }

    #[test]
    fn a_captured_cut_writes_only_to_its_event() {
        // A cut widget writes its selection once and deletes it; the write
        // must not also reach `writeText`.
        let sink = sink();
        let mut system = Vec::new();

        let capture = Capture::open(&sink).unwrap();

        if let Write::Passed(text) = sink.borrow_mut().write("cut".to_owned()) {
            system.push(text);
        }

        assert_eq!(capture.close(), Some(Captured("cut".to_owned())));
        assert!(system.is_empty());
    }

    #[test]
    fn the_last_write_of_a_turn_wins() {
        let sink = sink();
        let capture = Capture::open(&sink).unwrap();

        assert_eq!(sink.borrow_mut().write("first".to_owned()), Write::Captured);
        assert_eq!(
            sink.borrow_mut().write("second".to_owned()),
            Write::Captured
        );

        assert_eq!(capture.close(), Some(Captured("second".to_owned())));
    }

    #[test]
    fn a_capture_cannot_open_inside_another() {
        let sink = sink();
        let capture = Capture::open(&sink).unwrap();

        assert_eq!(Capture::open(&sink).unwrap_err(), Error::AlreadyOpen);

        let _ = capture.close();
        assert!(Capture::open(&sink).is_ok());
    }

    #[test]
    fn dropping_a_capture_closes_it() {
        let sink = sink();
        drop(Capture::open(&sink).unwrap());

        assert_eq!(
            sink.borrow_mut().write("text".to_owned()),
            Write::Passed("text".to_owned())
        );
    }
}
