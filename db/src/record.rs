use ulid::Ulid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBytes {
    id: Ulid,
    bytes: Vec<u8>,
}


impl RecordBytes {
    pub fn new(id: Ulid, bytes: Vec<u8>) -> Self {
        Self { id, bytes }
    }

    pub fn create(bytes: Vec<u8>) -> Self {
        Self {
            id: Ulid::new(),
            bytes
        }
    }

    pub fn id(&self) -> Ulid {
        self.id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
