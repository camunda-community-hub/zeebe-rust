use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;

mod extractor;
mod handler;

pub(crate) use extractor::FromJob;
pub use extractor::{Data, State};
pub(crate) use handler::HandlerFactory;

#[derive(Default)]
pub(crate) struct Extensions(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl Extensions {
    /// Build new default extensions
    pub(crate) fn new() -> Self {
        Extensions::default()
    }

    pub(crate) fn insert<T: Send + Sync + 'static>(&mut self, data: T) {
        self.0.insert(data.type_id(), Box::new(data));
    }

    pub(crate) fn get<T: 'static>(&self) -> Option<&T> {
        self.0
            .get(&TypeId::of::<T>())
            .and_then(|boxed| (&**boxed as &(dyn Any + 'static)).downcast_ref())
    }
}

impl fmt::Debug for Extensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Extensions {{ }}")
    }
}
