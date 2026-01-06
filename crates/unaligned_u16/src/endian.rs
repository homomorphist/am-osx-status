/// A module containing a collection of aliases to endian types.
/// These can be used when shorthand names may be desired.
pub mod aliases {
    pub use crate::endian::{BigEndian, LittleEndian, SystemEndian};
    pub use SystemEndian as System;
    pub use SystemEndian as Sys;
    pub use SystemEndian as system;
    pub use SystemEndian as sys;
    pub use LittleEndian as LE;
    pub use LittleEndian as le;
    pub use LittleEndian as Little;
    pub use LittleEndian as little;
    pub use BigEndian as BE;
    pub use BigEndian as be;
    pub use BigEndian as Big;
    pub use BigEndian as big;
}

pub struct SystemEndian;
pub struct LittleEndian;
pub struct BigEndian;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness { Little, Big }
impl Endianness {
    pub const SYSTEM: Self = Self::system();
    pub const LITTLE: Self = Self::Little;
    pub const BIG: Self = Self::Big;

    /// All possible endianness variants. Order is little-endian first, big-endian second.
    pub const VARIANTS: [Self; 2] = [Self::Little, Self::Big];

    /// Outputs the target system endianness at compile time.
    #[must_use]
    pub const fn system() -> Self {
        if cfg!(target_endian = "little") {
            Self::Little
        } else {
            Self::Big
        }
    }

    #[must_use] pub const fn little() -> Self { Self::Little }
    #[must_use] pub const fn big() -> Self { Self::Big }

    #[must_use] pub const fn is_system(self) -> bool { matches!(self, Self::SYSTEM) }
    #[must_use] pub const fn is_little(self) -> bool { matches!(self, Self::LITTLE) }
    #[must_use] pub const fn is_big(self) -> bool { matches!(self, Self::BIG) }

    /// Returns the opposite endianness variant.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Little => Self::Big,
            Self::Big    => Self::Little,
        }
    }

    #[must_use]
    pub const fn split_u16(self, value: u16) -> [u8; 2] {
        match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        }
    }

    #[must_use]
    pub const fn merge_bytes(self, bytes: [u8; 2]) -> u16 {
        match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        }
    }
}

pub trait Endian {
    const IS_SYSTEM: bool = cfg!(target_endian = "little") == Self::IS_LITTLE;
    const IS_LITTLE: bool;
    const IS_BIG: bool = !Self::IS_LITTLE;

    #[must_use]
    fn to_variant() -> Endianness {
        if Self::IS_LITTLE {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }
}

impl Endian for SystemEndian { const IS_LITTLE: bool = Endianness::SYSTEM.is_little(); }
impl Endian for LittleEndian { const IS_LITTLE: bool = true; }
impl Endian for BigEndian    { const IS_LITTLE: bool = false; }
