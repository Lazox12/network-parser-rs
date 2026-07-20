use crate::*;
use alloc::vec::Vec;

make_struct! { TestPeekMulti {
    field1: u16,
    if peek(16) == 0x8100 {
        field2: u16,
        field3: u16,
    }
}}

#[test]
fn test_peek_multi() {}
