use std::sync::Arc;

use bytepack::{Pack, Unpack};


#[derive(Debug, PartialEq, Eq, Clone, Pack, Unpack, Hash)]
pub struct Named<T> {
    pub name: Arc<str>,
    pub value: T,
}

impl<T> Named<T> {
    pub fn new(name: impl Into<Arc<str>>, value: T) -> Self {
        Self { name: name.into(), value }
    }
}