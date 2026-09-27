extern crate alloc;
use alloc::vec::Vec;
use network_parser_rs::*;

make_struct! { TestConsumeZeros {
    pub field1: u8,
    consume(16) // consumes 16 bits (2 bytes)
    pub field2: u8,
}}

#[test]
fn test_consume_writes_zeros() {
    let instance = TestConsumeZeros {
        field1: 0xAA,
        field2: 0xBB,
    };
    
    let mut buffer = Vec::new();
    let mut bit_offset = 0;
    instance.write_bits(&mut buffer, &mut bit_offset);
    
    // The struct should serialize to:
    // Byte 0: field1 (0xAA)
    // Byte 1: zeroed out by consume(16)
    // Byte 2: zeroed out by consume(16)
    // Byte 3: field2 (0xBB)
    assert_eq!(buffer.len(), 4);
    assert_eq!(buffer[0], 0xAA);
    assert_eq!(buffer[1], 0x00);
    assert_eq!(buffer[2], 0x00);
    assert_eq!(buffer[3], 0xBB);
}
