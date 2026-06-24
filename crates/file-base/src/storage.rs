use opendal::{Operator, services::Memory};

pub struct OpendalFileStorage {
    pub op: Operator,
}

impl OpendalFileStorage {
    pub fn new() -> Self {
        let op = Operator::new(Memory::default()).unwrap().finish();
        Self { op }
    }
}
