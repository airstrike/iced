use crate::menu::Output;

type Activate<Message> = dyn Fn(usize, &[usize]) -> Option<Output<Message>>;

/// A mapped prefix followed by entries appended after mapping.
pub(super) struct Prefix<Message> {
    pub len: usize,
    pub activate: Box<Activate<Message>>,
}
