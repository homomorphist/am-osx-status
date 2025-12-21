pub struct BigEndian;
pub struct LittleEndian;
pub struct SystemEndian;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}
impl Endianness {
    pub const fn is_little(&self) -> bool {
        matches!(self, Endianness::Little)
    }
    pub const fn is_big(&self) -> bool {
        matches!(self, Endianness::Big)
    }

    pub const SYSTEM: Endianness = Self::system();

    pub const fn system() -> Self {
        if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }
}

pub trait EndianIdentifier {
    const IS_LITTLE: bool;
    const IS_BIG: bool = !Self::IS_LITTLE;
    const IS_KNOWN: bool = true;

    fn to_variant() -> Endianness {
        if Self::IS_LITTLE {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }
}
impl EndianIdentifier for LittleEndian {
    const IS_LITTLE: bool = true;
}
impl EndianIdentifier for BigEndian {
    const IS_LITTLE: bool = false;
}
impl EndianIdentifier for SystemEndian {
    const IS_LITTLE: bool = Endianness::SYSTEM.is_little();
}
impl EndianIdentifier for () {
    const IS_BIG: bool = false;
    const IS_LITTLE: bool = false;
    const IS_KNOWN: bool = false;
}
