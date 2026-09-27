use network_parser_rs::*;

make_struct! { TestConsumeExpr {
    pub field1: u8,
    consume(field1 as usize * 8 * 2),
    pub field2: u8,
}}

#[test]
fn test_consume_expr() {
    let data = vec![2, 0, 0, 0, 0, 42];
    // field1 = 2 (consumes 1 byte)
    // consume 2 * 2 * 8 = 32 bits (4 bytes)
    // field2 should be read from offset 40 bits (byte index 5), which is 42
    let mut offset = 0;
    let res = TestConsumeExpr::parse_bits(&data, &mut offset).unwrap();
    assert_eq!(res.field1, 2);
    assert_eq!(res.field2, 42);
}
