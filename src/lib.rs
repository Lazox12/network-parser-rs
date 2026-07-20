#![cfg_attr(not(feature="std"), no_std)]
use core::convert::{TryFrom, From, Into};
extern crate alloc;
use alloc::vec::Vec;

pub trait NetworkParse: Sized + TryFrom<Vec<u8>> + Into<Vec<u8>> {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str>;
    fn write_bits(self, buffer: &mut Vec<u8>, bit_offset: &mut usize);
}

pub use network_parser_rs_macro::make_struct;

#[cfg(test)]
mod tests {
    use alloc::ffi::CString;
    use super::*;

    make_struct! { TestStruct,
        field1: u3,
        consume(5) // skips 5 bits (completes the first byte)
        field2: u5,
        consume(3) // padding to align Vec<u8> to byte boundary
        field3: Vec<u8>;field2, // vec of size defined in field2
        field4: CStr,
        if field1 == 7 {
            field5: u16,
            field6: [u8; 4],
        }
        field7: i12, // 12-bit signed
        consume(4)
        field8: [u8;4]
    }

    #[test]
    fn test_struct_fields_exist() {
        // This test ensures the struct is generated with the correct fields.
        let instance = TestStruct {
            field1: 7,
            field2: 3,
            field3: vec![1, 2, 3],
            field4: CString::new("test").unwrap(),
            field5: Some(1024),
            field6: Some([10, 20, 30, 40]),
            field7: -500,
            field8: [6,6,6,6],
        };
        
        assert_eq!(instance.field1, 7);
    }

    #[test]
    fn test_try_from_vec() {
        let data: Vec<u8> = vec![
            0b1110_0000, // byte 0: field1 = 7 (u3) = 111, consume(5) = 00000
            0b0001_1000, // byte 1: field2 = 3 (u5) = 00011, consume(3) = 000
            0x01, 0x02, 0x03, // byte 2-4: field3 (Vec<u8> of length field2 = 3)
            b't', b'e', b's', b't', 0x00, // byte 5-9: field4 (CStr)
            0x04, 0x00, // byte 10-11: field5 (u16) = 1024 (0x0400)
            10, 20, 30, 40, // byte 12-15: field6 ([u8; 4])
            0b1110_0000, 0b1100_0000, // byte 16-17: field7 = -500 (i12) -> 1110_0000_1100, then 4 bits padding
            6, 6, 6, 6, // byte 18-21: field8 ([u8; 4])
        ];
        
        let parsed = TestStruct::try_from(data).unwrap();
        assert_eq!(parsed.field1, 7);
        assert_eq!(parsed.field2, 3);
        assert_eq!(parsed.field3, vec![1, 2, 3]);
        assert_eq!(parsed.field4, CString::new("test").unwrap());
        assert_eq!(parsed.field5, Some(1024));
        assert_eq!(parsed.field6, Some([10, 20, 30, 40]));
        assert_eq!(parsed.field7, -500);
    }
    

    #[test]
    fn test_into_vec() {
        let instance = TestStruct {
            field1: 7,
            field2: 3,
            field3: vec![1, 2, 3],
            field4: CString::new("test").unwrap(),
            field5: Some(1024),
            field6: Some([10, 20, 30, 40]),
            field7: -500,
            field8: [6,6,6,6],
        };
        
        let data: Vec<u8> = instance.into();
        
        let expected_data: Vec<u8> = vec![
            0b1110_0000, // byte 0: field1 = 7 (u3) = 111, consume(5) = 00000
            0b0001_1000, // byte 1: field2 = 3 (u5) = 00011, consume(3) = 000
            0x01, 0x02, 0x03, // byte 2-4: field3 (Vec<u8> of length field2 = 3)
            b't', b'e', b's', b't', 0x00, // byte 5-9: field4 (CStr)
            0x04, 0x00, // byte 10-11: field5 (u16) = 1024 (0x0400)
            10, 20, 30, 40, // byte 12-15: field6 ([u8; 4])
            0b1110_0000, 0b1100_0000, // byte 16-17: field7 = -500 (i12) -> 1110_0000_1100, then 4 bits padding
            6, 6, 6, 6, // byte 18-21: field8 ([u8; 4])
        ];
        
        assert_eq!(data, expected_data);
    }

    make_struct! { 
        #[derive(Debug, Clone, PartialEq)]
        AttributeTest,
        field1: u8,
    }

    #[test]
    fn test_attributes() {
        let instance1 = AttributeTest { field1: 42 };
        let instance2 = instance1.clone();
        assert_eq!(instance1, instance2);
        assert_eq!(format!("{:?}", instance1), "AttributeTest { field1: 42 }");
    }

    make_struct! {
        #[derive(Debug, Clone, PartialEq)]
        BoxTestInner,
        inner_field: u8,
    }

    make_struct! {
        #[derive(Debug, Clone, PartialEq)]
        BoxTest,
        boxed_field: Box<BoxTestInner>,
    }

    #[test]
    fn test_box() {
        let instance = BoxTest {
            boxed_field: alloc::boxed::Box::new(BoxTestInner { inner_field: 42 }),
        };
        let mut buffer = Vec::new();
        let mut bit_offset = 0;
        instance.clone().write_bits(&mut buffer, &mut bit_offset);
        
        let mut read_offset = 0;
        let parsed = BoxTest::try_from(buffer).unwrap();
        assert_eq!(parsed, instance);
    }

    make_struct! {
        #[derive(Debug, Clone, PartialEq)]
        ExcludeTest,
        normal_field: u8,
        exclude {
            ignored_field: u16,
            another_ignored: bool,
        }
    }

    #[test]
    fn test_exclude() {
        let instance = ExcludeTest {
            normal_field: 42,
            ignored_field: 999, // Should be ignored during serialization
            another_ignored: true,
        };
        
        let mut buffer = Vec::new();
        let mut bit_offset = 0;
        instance.clone().write_bits(&mut buffer, &mut bit_offset);
        
        // Only normal_field should be serialized (1 byte)
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0], 42);
        
        // When deserializing, ignored fields should be Default::default()
        let mut read_offset = 0;
        let parsed = ExcludeTest::try_from(buffer).unwrap();
        assert_eq!(parsed.normal_field, 42);
        assert_eq!(parsed.ignored_field, 0); // Default for u16
        assert_eq!(parsed.another_ignored, false); // Default for bool
    }
}