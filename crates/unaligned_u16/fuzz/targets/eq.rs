#![no_main]

use libfuzzer_sys::fuzz_target;
use unaligned_u16::{u16_slice_as_u8_slice, utf16::Utf16Str, endian::SystemEndian};

fuzz_target!(|utf8: &str| {
    let u16_bytes = utf8.encode_utf16().collect::<Vec<_>>();
    let utf16 = Utf16Str::<SystemEndian>::new(u16_slice_as_u8_slice(&u16_bytes)).expect("couldn't convert left");
    assert!(utf8 == utf16);
});
