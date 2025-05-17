#![no_main]

use libfuzzer_sys::fuzz_target;
use unaligned_u16::{u16_slice_as_u8_slice, utf16::Utf16Str};

fuzz_target!(|v: (&str, &str)| {
    let (l, r) = v;
    let l_u16_bytes = l.encode_utf16().collect::<Vec<_>>();
    let r_u16_bytes = r.encode_utf16().collect::<Vec<_>>();
    let ll = Utf16Str::new(u16_slice_as_u8_slice(&l_u16_bytes)).expect("couldn't convert left");
    let rr = Utf16Str::new(u16_slice_as_u8_slice(&r_u16_bytes)).expect("couldn't convert right");
    assert!(l.cmp(r) == ll.cmp(rr));
    assert!(l == ll);
    assert!(r == rr);
});
