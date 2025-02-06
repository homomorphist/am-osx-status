#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct KilobitsPerSecond(pub u32);
impl AsRef<u32> for KilobitsPerSecond {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}
impl KilobitsPerSecond {
    pub fn into_inner(self) -> u32 {
        self.0
    }
}
impl From<KilobitsPerSecond> for u32 {
    fn from(val: KilobitsPerSecond) -> Self {
        val.0
    }
}
impl From<KilobitsPerSecond> for u64 {
    fn from(val: KilobitsPerSecond) -> Self {
        val.0 as u64
    }
}
