use crate::*;
use alloc::vec::Vec;

make_struct! { TestUSize {
    field1: usize,
    _field2: u8,
}}

#[test]
fn test_usize_works() {
    let mut data = Vec::new();
    let instance = TestUSize { field1: 1234567, _field2: 255 };
    let mut offset = 0;
    instance.write_bits(&mut data, &mut offset);
    
    let mut read_offset = 0;
    let parsed = TestUSize::parse_bits(&data, &mut read_offset).unwrap();
    assert_eq!(parsed.field1, 1234567);
    assert_eq!(parsed._field2, 255);
}
