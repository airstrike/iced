use super::{Dispatch, Output};

/// Owns one mapper around the original dispatch, regardless of its item count.
pub(super) struct Map<Message, F> {
    source: Dispatch<Message>,
    mapper: F,
}

impl<Message, Other, F> Map<Message, F>
where
    F: Fn(Message) -> Other,
{
    pub(super) fn new(source: Dispatch<Message>, mapper: F) -> Self {
        Self { source, mapper }
    }

    pub(super) fn activate(&self, index: usize, path: &[usize]) -> Option<Output<Other>> {
        match self.source.activate_at(index, path)? {
            Output::Message(message) => Some(Output::Message((self.mapper)(message))),
            Output::Edit(command) => Some(Output::Edit(command)),
        }
    }
}
