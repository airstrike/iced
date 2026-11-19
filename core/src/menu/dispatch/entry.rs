use crate::menu::{Action, Dispatch};

pub(in crate::menu) enum Entry<Message> {
    Item(Option<Action<Message>>),
    Menu(Dispatch<Message>),
    Separator,
}
