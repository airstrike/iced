//! Anchor an overlay to a widget: a popover, a menu, a hover card.
//!
//! [`Position`] is the placement math on its own: given the bounds of an
//! anchor, the size of some content and a boundary (usually the window), it
//! puts the content on a [`Side`] of the anchor with an [`Align`]ment and an
//! offset, flips it to the opposite side when it would leave the boundary,
//! shifts it along the anchor to stay inside, and finds where an arrow
//! pointing at the anchor goes. Any overlay can call [`Position::place`].
//!
//! [`Anchored`] is a widget built on it that owns its anchor: it lays out as
//! its anchor and, while open, shows its content in an overlay placed by its
//! [`Position`]. A press outside the content or `Escape` asks to close it
//! through [`Anchored::on_toggle`], and [`Anchored::hover`] makes it open
//! itself while the pointer rests on the anchor or the content.
//!
//! # Example
//! ```no_run
//! # mod iced { pub mod widget { pub use iced_widget::*; } pub use iced_widget::Renderer; pub use iced_widget::core::*; }
//! # use iced::widget::Widget;
//! # pub struct State { is_open: bool }
//! use iced::widget::{anchored, button, container};
//! use iced::widget::anchored::{Align, Side};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Toggle(bool),
//! }
//!
//! fn view(state: &State) -> impl Widget<Message> {
//!     anchored(
//!         button("Open popover").on_press(Message::Toggle(!state.is_open)),
//!         container("Place content for the popover here.").padding(10),
//!     )
//!     .side(Side::Bottom)
//!     .align(Align::Start)
//!     .offset(4)
//!     .open(state.is_open)
//!     .on_toggle(Message::Toggle)
//! }
//! ```
use crate::core::keyboard;
use crate::core::keyboard::key;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::time::{Duration, Instant};
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::{Meta, Operation};
use crate::core::window;
use crate::core::{
    Background, Color, Event, Length, Pixels, Point, Rectangle, Shell, Size, Vector, Widget,
};

/// The side of its anchor the content is placed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// Above the anchor.
    Top,
    /// To the right of the anchor.
    Right,
    /// Below the anchor.
    #[default]
    Bottom,
    /// To the left of the anchor.
    Left,
}

impl Side {
    /// The side across the anchor.
    pub fn opposite(self) -> Self {
        match self {
            Side::Top => Side::Bottom,
            Side::Right => Side::Left,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
        }
    }

    fn is_vertical(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// How the content lines up with its anchor along the anchor's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// The content starts where the anchor starts (its left or top edge).
    Start,
    /// The content is centered on the anchor.
    #[default]
    Center,
    /// The content ends where the anchor ends (its right or bottom edge).
    End,
}

/// Where content goes relative to an anchor, and how it keeps inside a
/// boundary.
///
/// The defaults follow Radix's popper: [`Side::Bottom`], [`Align::Center`],
/// no offsets, flipping and shifting on, no padding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    side: Side,
    align: Align,
    offset: f32,
    align_offset: f32,
    flip: bool,
    shift: bool,
    padding: f32,
    arrow: f32,
}

/// Where a [`Position`] put some content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    bounds: Rectangle,
    side: Side,
    arrow: Option<f32>,
}

/// How wide the content of an [`Anchored`] is.
///
/// The content is measured at its intrinsic width and then laid out at
/// exactly the width chosen here, so its `Fill` children span it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Width {
    /// As wide as the content wants.
    #[default]
    Content,
    /// Exactly as wide as the anchor (a combobox list).
    Anchor,
    /// At least as wide as the anchor (a select list).
    AtLeastAnchor,
}

impl Default for Position {
    fn default() -> Self {
        Self::new(Side::default())
    }
}

impl Position {
    /// Places content on the given [`Side`] of its anchor.
    pub fn new(side: Side) -> Self {
        Self {
            side,
            align: Align::Center,
            offset: 0.0,
            align_offset: 0.0,
            flip: true,
            shift: true,
            padding: 0.0,
            arrow: 0.0,
        }
    }

    /// Sets the side of the anchor the content prefers.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Sets how the content lines up with the anchor along its side.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets the gap between the anchor and the content.
    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.offset = offset.into().0;
        self
    }

    /// Moves start- or end-aligned content inward along the anchor by
    /// `offset`. Centered content ignores it, as in Radix.
    pub fn align_offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.align_offset = offset.into().0;
        self
    }

    /// Sets whether the content moves to the opposite side when it does not
    /// fit on its own.
    pub fn flip(mut self, flip: bool) -> Self {
        self.flip = flip;
        self
    }

    /// Sets whether the content slides along the anchor to stay inside the
    /// boundary. It never slides so far that it stops touching the anchor.
    pub fn shift(mut self, shift: bool) -> Self {
        self.shift = shift;
        self
    }

    /// Sets the distance the content keeps from the edges of the boundary.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = padding.into().0;
        self
    }

    /// Sets the width of an arrow at the content's edge; [`Placement::arrow`]
    /// is only given when an arrow this wide can point at the anchor's
    /// center.
    pub fn arrow(mut self, width: impl Into<Pixels>) -> Self {
        self.arrow = width.into().0;
        self
    }

    /// Returns the preferred [`Side`].
    pub fn preferred_side(&self) -> Side {
        self.side
    }

    /// Returns the largest size content can take next to `anchor` without
    /// leaving `boundary`: the boundary's extent across the side, and the
    /// room on the side itself (or on the roomier of the two sides, when the
    /// content may flip).
    pub fn available(&self, anchor: Rectangle, boundary: Rectangle) -> Size {
        let boundary = self.inner(boundary);
        let room = |side| self.room(side, anchor, boundary).max(0.0);

        let along = if self.flip {
            room(self.side).max(room(self.side.opposite()))
        } else {
            room(self.side)
        };

        if self.side.is_vertical() {
            Size::new(boundary.width, along)
        } else {
            Size::new(along, boundary.height)
        }
    }

    /// Places content of the given `size` next to `anchor`, inside
    /// `boundary`.
    pub fn place(&self, anchor: Rectangle, size: Size, boundary: Rectangle) -> Placement {
        let boundary = self.inner(boundary);

        let side = if self.flip {
            let preferred = self.overflow(self.side, anchor, size, boundary);
            let opposite = self.overflow(self.side.opposite(), anchor, size, boundary);

            if preferred > 0.0 && opposite < preferred {
                self.side.opposite()
            } else {
                self.side
            }
        } else {
            self.side
        };

        let main = self.main(side, anchor, size);
        let mut cross = self.cross(side, anchor, size);

        let (anchor_start, anchor_length, length, start, end) = if side.is_vertical() {
            (
                anchor.x,
                anchor.width,
                size.width,
                boundary.x,
                boundary.x + boundary.width,
            )
        } else {
            (
                anchor.y,
                anchor.height,
                size.height,
                boundary.y,
                boundary.y + boundary.height,
            )
        };

        if self.shift {
            // Clamp to the boundary, favoring its start when the content is
            // longer than the boundary, then keep the content touching the
            // anchor (Radix's `sticky="partial"`).
            cross = cross.min(end - length).max(start);
            cross = cross
                .max(anchor_start - length)
                .min(anchor_start + anchor_length);
        }

        let arrow = (self.arrow > 0.0)
            .then(|| {
                let center = anchor_start + anchor_length / 2.0 - cross;
                let half = self.arrow / 2.0;

                (center >= half && center <= length - half).then_some(center)
            })
            .flatten();

        let position = if side.is_vertical() {
            Point::new(cross, main)
        } else {
            Point::new(main, cross)
        };

        Placement {
            bounds: Rectangle::new(position, size),
            side,
            arrow,
        }
    }

    fn inner(&self, boundary: Rectangle) -> Rectangle {
        let padding = self
            .padding
            .min(boundary.width / 2.0)
            .min(boundary.height / 2.0)
            .max(0.0);

        boundary.shrink(padding)
    }

    /// The room between the anchor (plus the offset) and the boundary's edge
    /// on `side`.
    fn room(&self, side: Side, anchor: Rectangle, boundary: Rectangle) -> f32 {
        match side {
            Side::Top => anchor.y - self.offset - boundary.y,
            Side::Right => boundary.x + boundary.width - (anchor.x + anchor.width + self.offset),
            Side::Bottom => boundary.y + boundary.height - (anchor.y + anchor.height + self.offset),
            Side::Left => anchor.x - self.offset - boundary.x,
        }
    }

    /// How far content of `size` on `side` crosses the boundary's edge.
    fn overflow(&self, side: Side, anchor: Rectangle, size: Size, boundary: Rectangle) -> f32 {
        let length = if side.is_vertical() {
            size.height
        } else {
            size.width
        };

        (length - self.room(side, anchor, boundary)).max(0.0)
    }

    /// The content's coordinate across the side (y for top and bottom).
    fn main(&self, side: Side, anchor: Rectangle, size: Size) -> f32 {
        match side {
            Side::Top => anchor.y - self.offset - size.height,
            Side::Right => anchor.x + anchor.width + self.offset,
            Side::Bottom => anchor.y + anchor.height + self.offset,
            Side::Left => anchor.x - self.offset - size.width,
        }
    }

    /// The content's coordinate along the side (x for top and bottom).
    fn cross(&self, side: Side, anchor: Rectangle, size: Size) -> f32 {
        let (start, anchor_length, length) = if side.is_vertical() {
            (anchor.x, anchor.width, size.width)
        } else {
            (anchor.y, anchor.height, size.height)
        };

        match self.align {
            Align::Start => start + self.align_offset,
            Align::Center => start + (anchor_length - length) / 2.0,
            Align::End => start + anchor_length - length - self.align_offset,
        }
    }
}

impl Placement {
    /// The bounds of the content.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// The side of the anchor the content ended up on.
    pub fn side(&self) -> Side {
        self.side
    }

    /// The center of the arrow along the content's edge facing the anchor,
    /// measured from the content's left (top and bottom sides) or top (left
    /// and right sides); `None` when no arrow was asked for or it cannot
    /// point at the anchor's center from inside the content.
    pub fn arrow(&self) -> Option<f32> {
        self.arrow
    }
}

/// A widget showing content in an overlay anchored to it.
///
/// It is shown unless [`open`](Self::open) says otherwise, or, with
/// [`hover`](Self::hover), while the pointer rests on it.
pub struct Anchored<'a, W, V, Message, Theme = crate::Theme> {
    anchor: W,
    content: V,
    position: Position,
    point: Option<Point>,
    width: Width,
    min_width: f32,
    open: Option<bool>,
    hover: Option<Hover>,
    modal: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    arrow: Option<Arrow<'a, Theme>>,
}

/// The delays of hover intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hover {
    open: Duration,
    close: Duration,
}

/// The arrow drawn from the content's edge toward the anchor.
struct Arrow<'a, Theme> {
    size: Size,
    color: Box<dyn Fn(&Theme) -> Color + 'a>,
}

impl<'a, W, V, Message, Theme> Anchored<'a, W, V, Message, Theme> {
    /// Creates an [`Anchored`] showing `content` next to `anchor`.
    pub fn new(anchor: W, content: V) -> Self {
        Self {
            anchor,
            content,
            position: Position::default(),
            point: None,
            width: Width::Content,
            min_width: 0.0,
            open: None,
            hover: None,
            modal: false,
            on_toggle: None,
            arrow: None,
        }
    }

    /// Sets the whole [`Position`] at once.
    pub fn position(mut self, position: Position) -> Self {
        self.position = position.arrow(self.position.arrow);
        self
    }

    /// Sets the side of the anchor the content prefers.
    pub fn side(mut self, side: Side) -> Self {
        self.position = self.position.side(side);
        self
    }

    /// Sets how the content lines up with the anchor along its side.
    pub fn align(mut self, align: Align) -> Self {
        self.position = self.position.align(align);
        self
    }

    /// Sets the gap between the anchor and the content.
    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.position = self.position.offset(offset);
        self
    }

    /// Moves start- or end-aligned content inward along the anchor.
    pub fn align_offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.position = self.position.align_offset(offset);
        self
    }

    /// Sets whether the content flips to the opposite side when it does not
    /// fit.
    pub fn flip(mut self, flip: bool) -> Self {
        self.position = self.position.flip(flip);
        self
    }

    /// Sets whether the content slides along the anchor to stay in the
    /// window.
    pub fn shift(mut self, shift: bool) -> Self {
        self.position = self.position.shift(shift);
        self
    }

    /// Sets the distance the content keeps from the window's edges.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.position = self.position.padding(padding);
        self
    }

    /// Anchors the content to a point inside the anchor, relative to its
    /// top-left corner, instead of to its bounds (a context menu at the
    /// cursor).
    pub fn at(mut self, point: impl Into<Point>) -> Self {
        self.point = Some(point.into());
        self
    }

    /// Sets how wide the content is.
    pub fn width(mut self, width: Width) -> Self {
        self.width = width;
        self
    }

    /// Sets the narrowest the content may be laid out (a menu's `min-w-32`).
    pub fn min_width(mut self, min_width: impl Into<Pixels>) -> Self {
        self.min_width = min_width.into().0;
        self
    }

    /// Shows the content exactly when `open` is.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Sets the message published with the open state the [`Anchored`]
    /// asks for: `false` when the content is dismissed (a press outside it,
    /// `Escape`), and either way when hover intent changes.
    pub fn on_toggle(mut self, on_toggle: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    /// Opens the content once the pointer has rested on the anchor for
    /// `open_delay`, and closes it once the pointer has left both the
    /// anchor and the content for `close_delay`. Unless the
    /// [`Anchored`] is [`open`](Self::open)-controlled, it shows and hides
    /// the content itself.
    pub fn hover(mut self, open_delay: Duration, close_delay: Duration) -> Self {
        self.hover = Some(Hover {
            open: open_delay,
            close: close_delay,
        });
        self
    }

    /// Sets whether the content is modal: everything outside it ignores the
    /// pointer, and any press outside it (the anchor included) dismisses it.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Draws an arrow of the given size from the content's edge toward the
    /// anchor's center, when it fits. The arrow sits inside the
    /// [`offset`](Self::offset), so the offset should be at least its
    /// height.
    pub fn arrow(mut self, size: impl Into<Size>, color: impl Fn(&Theme) -> Color + 'a) -> Self {
        let size = size.into();

        self.position = self.position.arrow(size.width);
        self.arrow = Some(Arrow {
            size,
            color: Box::new(color),
        });
        self
    }

    fn is_shown(&self, state: &State) -> bool {
        match (self.open, self.hover) {
            (Some(open), _) => open,
            (None, Some(_)) => state.intent.is_open(),
            (None, None) => true,
        }
    }

    /// Whether the content's tree is kept: always with hover intent, which
    /// decides at event time; otherwise only while open, so it mounts fresh
    /// each time it opens.
    fn keeps_content(&self) -> bool {
        self.hover.is_some() || self.open != Some(false)
    }
}

/// The hover intent of an [`Anchored`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Intent {
    #[default]
    Idle,
    Entering(Instant),
    Open,
    Leaving(Instant),
    /// Dismissed while hovered: stays closed until the pointer leaves.
    Dismissed,
}

impl Intent {
    fn is_open(self) -> bool {
        matches!(self, Intent::Open | Intent::Leaving(_))
    }
}

#[derive(Debug, Default)]
struct State {
    intent: Intent,
    over_anchor: bool,
    over_content: bool,
}

impl State {
    /// Advances the hover intent, publishing the change and invalidating
    /// the overlay when the content opens or closes.
    fn step<Message>(
        &mut self,
        hover: Hover,
        controlled: bool,
        on_toggle: Option<&dyn Fn(bool) -> Message>,
        shell: &mut Shell<'_, Message>,
    ) {
        let now = Instant::now();
        let hovered = self.over_anchor || self.over_content;
        let was_open = self.intent.is_open();

        self.intent = match (self.intent, hovered) {
            (Intent::Idle, true) if hover.open.is_zero() => Intent::Open,
            (Intent::Idle, true) => {
                shell.request_redraw_at(now + hover.open);
                Intent::Entering(now)
            }
            (Intent::Entering(at), true) => {
                if at.elapsed() >= hover.open {
                    Intent::Open
                } else {
                    shell.request_redraw_at(at + hover.open);
                    Intent::Entering(at)
                }
            }
            (Intent::Open | Intent::Leaving(_), true) => Intent::Open,
            (Intent::Open, false) if hover.close.is_zero() => Intent::Idle,
            (Intent::Open, false) => {
                shell.request_redraw_at(now + hover.close);
                Intent::Leaving(now)
            }
            (Intent::Leaving(at), false) => {
                if at.elapsed() >= hover.close {
                    Intent::Idle
                } else {
                    shell.request_redraw_at(at + hover.close);
                    Intent::Leaving(at)
                }
            }
            (Intent::Idle | Intent::Entering(_) | Intent::Dismissed, false) => Intent::Idle,
            (Intent::Dismissed, true) => Intent::Dismissed,
        };

        let is_open = self.intent.is_open();

        if is_open != was_open {
            if let Some(on_toggle) = on_toggle {
                shell.publish(on_toggle(is_open));
            }

            if !controlled {
                shell.invalidate_overlay();
                shell.request_redraw();
            }
        }
    }
}

impl<W, V, Message, Theme> Meta for Anchored<'_, W, V, Message, Theme> {}

impl<W, V, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Anchored<'_, W, V, Message, Theme>
where
    Renderer: crate::core::Renderer,
    W: Widget<Message, Theme, Renderer>,
    V: Widget<Message, Theme, Renderer>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        let children = if self.keeps_content() { 2 } else { 1 };

        tree.children.truncate(children);

        if tree.children.is_empty() {
            tree.children.push(Tree::new(&self.anchor));
        }

        if children == 2 && tree.children.len() == 1 {
            tree.children.push(Tree::new(&self.content));
        }

        tree.children[0].diff(&mut self.anchor);

        if children == 2 {
            tree.children[1].diff(&mut self.content);
        }
    }

    fn size(&self) -> Size<Length> {
        self.anchor.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.anchor.layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
        tree.children[0].translation = Vector::ZERO;
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        {
            let (anchor_layout, anchor_tree) = layout
                .iter_mut(&mut tree.children[..1])
                .next()
                .expect("an anchored widget has its anchor");

            self.anchor.update(
                anchor_tree,
                event,
                anchor_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
        }

        let Some(hover) = self.hover else {
            return;
        };

        let is_shown = self.is_shown(tree.state.downcast_ref::<State>());
        let state = tree.state.downcast_mut::<State>();

        if !is_shown {
            state.over_content = false;
        }

        match event {
            Event::Mouse(_) | Event::Touch(_) => {
                state.over_anchor = cursor.is_over(layout.bounds());
            }
            Event::Window(window::Event::RedrawRequested(_)) => {}
            _ => return,
        }

        state.step(hover, self.open.is_some(), self.on_toggle.as_deref(), shell);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let (anchor_layout, anchor_tree) = layout
            .iter(&tree.children[..1])
            .next()
            .expect("an anchored widget has its anchor");

        self.anchor.draw(
            anchor_tree,
            renderer,
            theme,
            style,
            anchor_layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (anchor_layout, anchor_tree) = layout
            .iter(&tree.children[..1])
            .next()
            .expect("an anchored widget has its anchor");

        self.anchor
            .mouse_interaction(anchor_tree, anchor_layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (anchor_layout, anchor_tree) = layout
            .iter_mut(&mut tree.children[..1])
            .next()
            .expect("an anchored widget has its anchor");

        self.anchor
            .operate(anchor_tree, anchor_layout, viewport, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let is_shown = self.is_shown(tree.state.downcast_ref::<State>());
        let state = tree.state.downcast_mut::<State>();
        let (anchor_tree, content_tree) = tree.children.split_at_mut(1);

        let (anchor_layout, anchor_tree) = layout
            .iter_mut(anchor_tree)
            .next()
            .expect("an anchored widget has its anchor");

        let mut overlays = self.anchor.overlay(
            anchor_tree,
            anchor_layout,
            renderer,
            viewport,
            translation,
            window,
        );

        let Some(content_tree) = content_tree.first_mut().filter(|_| is_shown) else {
            return overlays;
        };

        let anchor = layout.bounds() + translation;
        let target = match self.point {
            Some(point) => Rectangle::new(
                anchor.position() + Vector::new(point.x, point.y),
                Size::ZERO,
            ),
            None => anchor,
        };
        let boundary = Rectangle::with_size(window);
        let available = self.position.available(target, boundary);

        // The content is measured at its intrinsic width first (fluid
        // lengths compressed), then laid out again at exactly the width it
        // gets, so `Fill` children span the whole panel, as `w-full` rows
        // do in a `w-fit` popover.
        let min_width = match self.width {
            Width::Content => self.min_width,
            Width::Anchor | Width::AtLeastAnchor => self.min_width.max(anchor.width),
        }
        .min(available.width);

        let width = match self.width {
            Width::Anchor => min_width,
            Width::Content | Width::AtLeastAnchor => {
                self.content.layout(
                    content_tree,
                    renderer,
                    &layout::Limits::with_flags(
                        Size::new(min_width, 0.0),
                        available,
                        Size::new(true, false),
                        Size::new(false, false),
                    ),
                );

                content_tree.size.width.max(min_width).min(available.width)
            }
        };

        self.content.layout(
            content_tree,
            renderer,
            &layout::Limits::new(Size::new(width, 0.0), Size::new(width, available.height)),
        );

        let placement = self.position.place(target, content_tree.size, boundary);

        overlays.push(overlay::Element::new(Box::new(Overlay {
            content: &mut self.content,
            tree: content_tree,
            state,
            layout: Layout::new(placement.bounds().size()).move_to(placement.bounds().position()),
            anchor,
            bridge: bridge(target, placement),
            placement,
            window,
            modal: self.modal,
            hover: self.hover,
            controlled: self.open.is_some(),
            on_toggle: self.on_toggle.as_deref(),
            arrow: self.arrow.as_ref(),
        })));

        overlays
    }
}

/// The open content of an [`Anchored`].
struct Overlay<'a, 'b, V, Message, Theme> {
    content: &'b mut V,
    tree: &'b mut Tree,
    state: &'b mut State,
    layout: Layout,
    anchor: Rectangle,
    bridge: Rectangle,
    placement: Placement,
    window: Size,
    modal: bool,
    hover: Option<Hover>,
    controlled: bool,
    on_toggle: Option<&'b (dyn Fn(bool) -> Message + 'a)>,
    arrow: Option<&'b Arrow<'a, Theme>>,
}

impl<V, Message, Theme> Overlay<'_, '_, V, Message, Theme> {
    fn dismiss(&mut self, shell: &mut Shell<'_, Message>) {
        if self.hover.is_some() && self.state.intent.is_open() {
            self.state.intent = Intent::Dismissed;

            if !self.controlled {
                shell.invalidate_overlay();
                shell.request_redraw();
            }
        }

        if let Some(on_toggle) = self.on_toggle {
            shell.publish(on_toggle(false));
        }
    }
}

impl<V, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, V, Message, Theme>
where
    Renderer: crate::core::Renderer,
    V: Widget<Message, Theme, Renderer>,
{
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: mouse::Cursor,
    ) {
        let viewport = Rectangle::with_size(self.window);

        renderer.with_layer(viewport, |renderer| {
            self.content.draw(
                self.tree,
                renderer,
                theme,
                style,
                self.layout,
                cursor,
                &viewport,
            );

            if let (Some(arrow), Some(center)) = (self.arrow, self.placement.arrow()) {
                draw_arrow(
                    renderer,
                    self.placement,
                    center,
                    arrow.size,
                    (arrow.color)(theme),
                );
            }
        });
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn Operation) {
        self.content.operate(
            self.tree,
            self.layout,
            &Rectangle::with_size(self.window),
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let viewport = Rectangle::with_size(self.window);
        let bounds = self.layout.bounds();

        self.content.update(
            self.tree,
            event,
            self.layout,
            cursor,
            renderer,
            shell,
            &viewport,
        );

        if let Some(hover) = self.hover
            && let Event::Mouse(_) | Event::Touch(_) = event
        {
            self.state.over_anchor = cursor.is_over(self.anchor);
            self.state.over_content = cursor.is_over(bounds) || cursor.is_over(self.bridge);
            self.state
                .step(hover, self.controlled, self.on_toggle, shell);
        }

        let is_over = cursor.is_over(bounds);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if !is_over && (self.modal || !cursor.is_over(self.anchor)) =>
            {
                self.dismiss(shell);
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(key::Named::Escape),
                ..
            }) if !shell.is_event_captured() => {
                self.dismiss(shell);
                shell.capture_event();
            }
            _ => {}
        }

        // The content is a layer: what it covers never sees the pointer's
        // presses or wheel, and a modal layer covers the window.
        let is_pointer = matches!(event, Event::Mouse(_) | Event::Touch(_));
        let covers = is_over
            && matches!(
                event,
                Event::Mouse(
                    mouse::Event::ButtonPressed(_)
                        | mouse::Event::ButtonReleased(_)
                        | mouse::Event::WheelScrolled { .. }
                ) | Event::Touch(_)
            );

        if covers || (self.modal && is_pointer) {
            shell.capture_event();
        }
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        let interaction = self.content.mouse_interaction(
            self.tree,
            self.layout,
            cursor,
            &Rectangle::with_size(self.window),
            renderer,
        );

        if interaction == mouse::Interaction::None
            && (self.modal || cursor.is_over(self.layout.bounds()))
        {
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }

    fn overlay<'c>(
        &'c mut self,
        renderer: &Renderer,
    ) -> Vec<overlay::Element<'c, Message, Theme, Renderer>> {
        self.content.overlay(
            self.tree,
            self.layout,
            renderer,
            &Rectangle::with_size(self.window),
            Vector::ZERO,
            self.window,
        )
    }
}

/// The band between the anchor and the placed content, which counts as
/// hovering so the pointer can cross the gap.
fn bridge(anchor: Rectangle, placement: Placement) -> Rectangle {
    let content = placement.bounds();

    let (start, end) = match placement.side() {
        Side::Top => (
            Point::new(anchor.x.min(content.x), content.y + content.height),
            Point::new(
                (anchor.x + anchor.width).max(content.x + content.width),
                anchor.y,
            ),
        ),
        Side::Bottom => (
            Point::new(anchor.x.min(content.x), anchor.y + anchor.height),
            Point::new(
                (anchor.x + anchor.width).max(content.x + content.width),
                content.y,
            ),
        ),
        Side::Left => (
            Point::new(content.x + content.width, anchor.y.min(content.y)),
            Point::new(
                anchor.x,
                (anchor.y + anchor.height).max(content.y + content.height),
            ),
        ),
        Side::Right => (
            Point::new(anchor.x + anchor.width, anchor.y.min(content.y)),
            Point::new(
                content.x,
                (anchor.y + anchor.height).max(content.y + content.height),
            ),
        ),
    };

    Rectangle::new(
        start,
        Size::new((end.x - start.x).max(0.0), (end.y - start.y).max(0.0)),
    )
}

/// Draws a triangle from the content's edge toward the anchor, in half
/// pixel rows (the renderer cannot rotate a quad).
fn draw_arrow<Renderer>(
    renderer: &mut Renderer,
    placement: Placement,
    center: f32,
    size: Size,
    color: Color,
) where
    Renderer: crate::core::Renderer,
{
    const ROW: f32 = 0.5;

    let content = placement.bounds();
    let rows = (size.height / ROW).ceil() as usize;

    for row in 0..rows {
        let distance = row as f32 * ROW;
        let width = size.width * (1.0 - distance / size.height);

        let bounds = match placement.side() {
            Side::Bottom => Rectangle::new(
                Point::new(content.x + center - width / 2.0, content.y - distance - ROW),
                Size::new(width, ROW),
            ),
            Side::Top => Rectangle::new(
                Point::new(
                    content.x + center - width / 2.0,
                    content.y + content.height + distance,
                ),
                Size::new(width, ROW),
            ),
            Side::Right => Rectangle::new(
                Point::new(content.x - distance - ROW, content.y + center - width / 2.0),
                Size::new(ROW, width),
            ),
            Side::Left => Rectangle::new(
                Point::new(
                    content.x + content.width + distance,
                    content.y + center - width / 2.0,
                ),
                Size::new(ROW, width),
            ),
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            Background::Color(color),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Rectangle = Rectangle::new(Point::ORIGIN, Size::new(400.0, 300.0));
    const ANCHOR: Rectangle = Rectangle::new(Point::new(100.0, 100.0), Size::new(80.0, 20.0));
    const CONTENT: Size = Size::new(120.0, 60.0);

    fn place(position: Position, anchor: Rectangle, size: Size) -> Placement {
        position.place(anchor, size, WINDOW)
    }

    #[test]
    fn bottom_center_sits_below_the_anchor_centered() {
        let placement = place(Position::new(Side::Bottom).offset(4), ANCHOR, CONTENT);

        assert_eq!(placement.side(), Side::Bottom);
        assert_eq!(placement.bounds().position(), Point::new(80.0, 124.0));
    }

    #[test]
    fn each_side_places_the_content_across_its_edge() {
        let at = |side| place(Position::new(side).offset(4).flip(false), ANCHOR, CONTENT).bounds();

        assert_eq!(at(Side::Top).y, 100.0 - 4.0 - 60.0);
        assert_eq!(at(Side::Right).x, 180.0 + 4.0);
        assert_eq!(at(Side::Bottom).y, 120.0 + 4.0);
        assert_eq!(at(Side::Left).x, 100.0 - 4.0 - 120.0);
        assert_eq!(at(Side::Right).y, 100.0 + (20.0 - 60.0) / 2.0);
    }

    #[test]
    fn start_and_end_line_up_with_the_anchor_edges() {
        let start = place(
            Position::new(Side::Bottom).align(Align::Start),
            ANCHOR,
            CONTENT,
        );
        let end = place(
            Position::new(Side::Bottom).align(Align::End),
            ANCHOR,
            CONTENT,
        );

        assert_eq!(start.bounds().x, 100.0);
        assert_eq!(end.bounds().x + end.bounds().width, 180.0);
    }

    #[test]
    fn align_offset_moves_inward_and_is_ignored_when_centered() {
        let position = Position::new(Side::Bottom).align_offset(10);

        assert_eq!(
            place(position.align(Align::Start), ANCHOR, CONTENT)
                .bounds()
                .x,
            110.0
        );
        assert_eq!(
            place(position.align(Align::End), ANCHOR, CONTENT)
                .bounds()
                .x,
            180.0 - 120.0 - 10.0
        );
        assert_eq!(
            place(position.align(Align::Center), ANCHOR, CONTENT)
                .bounds()
                .x,
            80.0
        );
    }

    #[test]
    fn flips_when_the_preferred_side_overflows_and_the_opposite_fits() {
        let anchor = Rectangle::new(Point::new(100.0, 260.0), Size::new(80.0, 20.0));
        let placement = place(Position::new(Side::Bottom).offset(4), anchor, CONTENT);

        assert_eq!(placement.side(), Side::Top);
        assert_eq!(placement.bounds().y, 260.0 - 4.0 - 60.0);
    }

    #[test]
    fn keeps_its_side_when_the_opposite_overflows_more() {
        let tall = Size::new(120.0, 200.0);
        let anchor = Rectangle::new(Point::new(100.0, 150.0), Size::new(80.0, 20.0));
        let placement = place(Position::new(Side::Bottom), anchor, tall);

        // 130 below, 150 above: both overflow, the top by less.
        assert_eq!(placement.side(), Side::Top);

        let anchor = Rectangle::new(Point::new(100.0, 90.0), Size::new(80.0, 20.0));
        let placement = place(Position::new(Side::Bottom), anchor, tall);

        assert_eq!(placement.side(), Side::Bottom, "190 below beats 90 above");
    }

    #[test]
    fn never_flips_when_flipping_is_off() {
        let anchor = Rectangle::new(Point::new(100.0, 260.0), Size::new(80.0, 20.0));
        let placement = place(Position::new(Side::Bottom).flip(false), anchor, CONTENT);

        assert_eq!(placement.side(), Side::Bottom);
    }

    #[test]
    fn shifts_along_the_anchor_to_stay_inside() {
        let anchor = Rectangle::new(Point::new(0.0, 100.0), Size::new(40.0, 20.0));
        let placement = place(Position::new(Side::Bottom), anchor, CONTENT);

        assert_eq!(placement.bounds().x, 0.0);

        let anchor = Rectangle::new(Point::new(370.0, 100.0), Size::new(30.0, 20.0));
        let placement = place(Position::new(Side::Bottom), anchor, CONTENT);

        assert_eq!(placement.bounds().x + placement.bounds().width, 400.0);
    }

    #[test]
    fn padding_keeps_the_content_off_the_edges() {
        let anchor = Rectangle::new(Point::new(0.0, 100.0), Size::new(40.0, 20.0));
        let placement = place(Position::new(Side::Bottom).padding(8), anchor, CONTENT);

        assert_eq!(placement.bounds().x, 8.0);
    }

    #[test]
    fn shifting_never_detaches_the_content_from_the_anchor() {
        // An anchor hanging off the left edge: clamping into the window
        // alone would leave a gap between the content and the anchor.
        let anchor = Rectangle::new(Point::new(-200.0, 100.0), Size::new(40.0, 20.0));
        let placement = place(Position::new(Side::Bottom), anchor, CONTENT);

        assert_eq!(placement.bounds().x, -160.0);
    }

    #[test]
    fn content_wider_than_the_boundary_starts_at_its_start() {
        let wide = Size::new(500.0, 60.0);
        let placement = place(Position::new(Side::Bottom), ANCHOR, wide);

        assert_eq!(placement.bounds().x, 0.0);
    }

    #[test]
    fn no_shift_leaves_the_content_where_it_is_aligned() {
        let anchor = Rectangle::new(Point::new(0.0, 100.0), Size::new(40.0, 20.0));
        let placement = place(Position::new(Side::Bottom).shift(false), anchor, CONTENT);

        assert_eq!(placement.bounds().x, -40.0);
    }

    #[test]
    fn arrow_points_at_the_anchor_center() {
        let placement = place(Position::new(Side::Bottom).arrow(10), ANCHOR, CONTENT);

        assert_eq!(placement.arrow(), Some(60.0));

        let start = place(
            Position::new(Side::Bottom).align(Align::Start).arrow(10),
            ANCHOR,
            CONTENT,
        );

        assert_eq!(start.arrow(), Some(40.0));
    }

    #[test]
    fn arrow_is_dropped_when_it_cannot_reach_the_anchor_center() {
        // Shifted so far that the anchor's center lies outside the content.
        let anchor = Rectangle::new(Point::new(-38.0, 100.0), Size::new(40.0, 20.0));
        let placement = place(Position::new(Side::Bottom).arrow(10), anchor, CONTENT);

        assert_eq!(placement.arrow(), None);
        assert_eq!(
            place(Position::new(Side::Bottom), ANCHOR, CONTENT).arrow(),
            None,
            "no arrow asked for"
        );
    }

    #[test]
    fn a_point_anchor_places_the_content_at_the_point() {
        let point = Rectangle::new(Point::new(250.0, 50.0), Size::ZERO);
        let placement = place(
            Position::new(Side::Bottom).align(Align::Start),
            point,
            CONTENT,
        );

        assert_eq!(placement.bounds().position(), Point::new(250.0, 50.0));

        let corner = Rectangle::new(Point::new(350.0, 280.0), Size::ZERO);
        let placement = place(
            Position::new(Side::Bottom).align(Align::Start),
            corner,
            CONTENT,
        );

        assert_eq!(placement.side(), Side::Top);
        assert_eq!(placement.bounds().position(), Point::new(280.0, 220.0));
    }

    #[test]
    fn available_is_the_roomier_side_when_flipping() {
        let position = Position::new(Side::Bottom).offset(4);

        assert_eq!(
            position.available(ANCHOR, WINDOW),
            Size::new(400.0, 300.0 - 120.0 - 4.0)
        );

        let anchor = Rectangle::new(Point::new(100.0, 260.0), Size::new(80.0, 20.0));

        assert_eq!(
            position.available(anchor, WINDOW),
            Size::new(400.0, 260.0 - 4.0)
        );
        assert_eq!(
            position.flip(false).available(anchor, WINDOW),
            Size::new(400.0, 16.0)
        );
    }
}
