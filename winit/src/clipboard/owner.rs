//! The elements of the page whose clipboard events iced answers.
use super::target::Target;

use crate::core::window;

use std::collections::BTreeMap;

use web_sys::wasm_bindgen::JsValue;

/// The elements of one iced window: its canvas and, when the window has one,
/// the element that holds the DOM focus for its input method.
#[derive(Debug, Clone)]
pub struct Owner {
    canvas: web_sys::HtmlCanvasElement,
    input_method: Option<web_sys::Element>,
}

/// The [`Owner`] of each open iced window.
#[derive(Debug, Default)]
pub struct Registry {
    owners: BTreeMap<window::Id, Owner>,
}

impl Owner {
    /// The owner of a window drawn on `canvas`.
    pub fn new(canvas: web_sys::HtmlCanvasElement) -> Self {
        Self {
            canvas,
            input_method: None,
        }
    }

    /// Adds the element that holds the DOM focus for the window's input
    /// method, whose clipboard events then belong to the window.
    #[allow(dead_code, reason = "a web build with a DOM input method calls it")]
    pub fn with_input_method(self, element: web_sys::Element) -> Self {
        Self {
            input_method: Some(element),
            ..self
        }
    }

    fn target(&self, element: &JsValue) -> Option<Target<web_sys::HtmlCanvasElement>> {
        if element == AsRef::<JsValue>::as_ref(&self.canvas) {
            return Some(Target::Canvas(self.canvas.clone()));
        }

        let input_method = self.input_method.as_ref()?;

        (element == AsRef::<JsValue>::as_ref(input_method))
            .then(|| Target::InputMethod(self.canvas.clone()))
    }
}

impl Registry {
    /// Answers the clipboard events of `owner` for the window `id`.
    pub fn insert(&mut self, id: window::Id, owner: Owner) {
        let _ = self.owners.insert(id, owner);
    }

    /// Leaves the clipboard events of the window `id` to the page.
    pub fn remove(&mut self, id: window::Id) {
        let _ = self.owners.remove(&id);
    }

    /// Where `event` lands.
    pub fn target(&self, event: &web_sys::Event) -> Target<web_sys::HtmlCanvasElement> {
        let Some(element) = event.target() else {
            return Target::Page;
        };

        let element: &JsValue = element.as_ref();

        self.owners
            .values()
            .find_map(|owner| owner.target(element))
            .unwrap_or(Target::Page)
    }
}
