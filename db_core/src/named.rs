use std::sync::Arc;


#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Named<T> {
    pub name: Arc<str>,
    pub value: T,
}
