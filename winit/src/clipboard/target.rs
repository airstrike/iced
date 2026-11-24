//! The element of the page a clipboard or key event lands on.

/// Where a DOM event lands, as the web clipboard sees it.
///
/// `C` is the canvas of the iced window the element belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target<C> {
    /// The canvas of an iced window.
    Canvas(C),
    /// The element that holds the DOM focus for the input method of an iced
    /// window while one of its text fields is edited.
    InputMethod(C),
    /// Any other element of the page, whose clipboard events are its own.
    Page,
}

impl<C> Target<C> {
    /// The canvas of the iced window the target belongs to, or `None` for an
    /// element of the page.
    pub fn into_canvas(self) -> Option<C> {
        match self {
            Target::Canvas(canvas) | Target::InputMethod(canvas) => Some(canvas),
            Target::Page => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_input_method_belongs_to_its_canvas() {
        assert_eq!(Target::InputMethod("canvas").into_canvas(), Some("canvas"));
        assert_eq!(Target::Canvas("canvas").into_canvas(), Some("canvas"));
        assert_eq!(Target::<&str>::Page.into_canvas(), None);
    }
}
