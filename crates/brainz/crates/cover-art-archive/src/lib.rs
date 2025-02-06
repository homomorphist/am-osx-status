pub struct Id(u64);
impl Id {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}
impl From<u64> for Id {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<Id> for u64 {
    fn from(value: Id) -> Self {
        value.into_inner()
    }
}
