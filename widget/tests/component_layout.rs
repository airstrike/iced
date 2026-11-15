//! Component layout caching must account for renderer defaults.
use iced_widget::core::Widget as _;
use iced_widget::core::renderer::{self, Renderer as _};
use iced_widget::core::widget::{Meta, Tree};
use iced_widget::core::{
    self, Background, Font, Layout, Length, Pixels, Rectangle, Size, Transformation, image, layout,
    mouse, text,
};
use iced_widget::{Component, Widget, component};

use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn component_relayouts_when_renderer_defaults_change_at_the_same_bounds() {
    let layouts = Rc::new(RefCell::new(Vec::new()));
    let mut component = component(Recorder(layouts.clone()));
    let mut tree = Tree::new(&component);
    tree.diff(&mut component);

    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
    let defaults = renderer::Settings::default();
    let settings = [
        defaults,
        renderer::Settings {
            font: Font::new("Edu SA Hand"),
            ..defaults
        },
        renderer::Settings {
            text_size: Pixels(24.0),
            ..defaults
        },
        renderer::Settings {
            line_height: text::LineHeight::Relative(2.0),
            ..defaults
        },
        renderer::Settings {
            metrics_hinting: !defaults.metrics_hinting,
            ..defaults
        },
    ];

    for (index, settings) in settings.into_iter().enumerate() {
        // Replacing the renderer mirrors font::set_defaults in the runtime.
        let renderer = Renderer(settings);
        component.layout(&mut tree, &renderer, &limits);
        assert_eq!(layouts.borrow().len(), index + 1);
        assert_eq!(layouts.borrow().last(), Some(&settings));
        assert_eq!(tree.size, Size::new(settings.text_size.0, 20.0));

        component.layout(&mut tree, &renderer, &limits);
        assert_eq!(
            layouts.borrow().len(),
            index + 1,
            "unchanged inputs stay cached"
        );
    }

    let renderer = Renderer(settings[settings.len() - 1]);
    let limits = layout::Limits::new(Size::ZERO, Size::new(500.0, 300.0));
    component.layout(&mut tree, &renderer, &limits);
    assert_eq!(layouts.borrow().len(), settings.len() + 1);
}

struct Recorder(Rc<RefCell<Vec<renderer::Settings>>>);

impl<'a> Component<'a, (), (), Renderer> for Recorder {
    type State = ();
    type Event = ();

    fn update(&self, _state: &mut (), _event: (), _renderer: &Renderer) -> Option<()> {
        None
    }

    fn view(&self, _state: &()) -> impl Widget<(), (), Renderer> + 'a {
        Self(self.0.clone())
    }
}

impl Meta for Recorder {}

impl core::Widget<(), (), Renderer> for Recorder {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, _limits: &layout::Limits) {
        self.0.borrow_mut().push(renderer.settings());
        tree.size = Size::new(renderer.settings().text_size.0, 20.0);
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &(),
        _style: &renderer::Style,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

struct Renderer(renderer::Settings);

impl core::Renderer for Renderer {
    fn start_layer(&mut self, _bounds: Rectangle) {}

    fn end_layer(&mut self) {}

    fn start_transformation(&mut self, _transformation: Transformation) {}

    fn end_transformation(&mut self) {}

    fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}

    fn allocate_image(
        &self,
        handle: &image::Handle,
        callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        ().allocate_image(handle, callback);
    }

    fn hint(&mut self, _scale: renderer::Scale) {}

    fn scale(&self) -> Option<renderer::Scale> {
        None
    }

    fn reset(&mut self, _bounds: Rectangle) {}

    fn settings(&self) -> renderer::Settings {
        self.0
    }
}
