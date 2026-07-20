#![cfg_attr(not(feature="std"), no_std)]
use core::convert::{TryFrom, From, Into};
extern crate alloc;
use alloc::vec::Vec;

pub trait NetworkParse: Sized + TryFrom<Vec<u8>> + Into<Vec<u8>> {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str>;
    fn write_bits(self, buffer: &mut Vec<u8>, bit_offset: &mut usize);
}

pub use network_parser_rs_macro::{make_struct, make_enum};

#[cfg(test)]
mod tests {
    use alloc::ffi::CString;
    use super::*;

    make_struct! { TestStruct {
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
    }}

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

    make_struct! { #[derive(Debug, Clone, PartialEq)] AttributeTest {
        field1: u8,
    }}

    #[test]
    fn test_attributes() {
        let instance1 = AttributeTest { field1: 42 };
        let instance2 = instance1.clone();
        assert_eq!(instance1, instance2);
        assert_eq!(format!("{:?}", instance1), "AttributeTest { field1: 42 }");
    }

    make_struct! {
        #[derive(Debug, Clone, PartialEq)]
        BoxTestInner {
            inner_field: u8,
        }
    }

    make_struct! {
        #[derive(Debug, Clone, PartialEq)]
        BoxTest {
            boxed_field: Box<BoxTestInner>,
        }
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
        ExcludeTest {
            normal_field: u8,
            exclude {
                ignored_field: u16,
                another_ignored: bool,
            }
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

    make_enum! {
        #[derive(Debug, Clone, PartialEq)]
        TestEnum: u16 {
            field1 == 123,
            field2(u8) == 542,
            field5(Vec<u8>) = vec![1, 2, 3] if tag_value == 555,
            field3(u16) < 1524,
            field4(u16) _
        }
    }

    make_enum! {
        #[derive(Clone, Debug, PartialEq)]
        EthIpType: Vec<u8> {
            IPv4(Vec<u8>) = self.clone() if self.len() == 4,
            IPv6(Vec<u8>) _
        }
    }

    #[test]
    fn test_enum() {
        let mut buffer = Vec::new();
        let mut bit_offset = 0;
        let e1 = TestEnum::field1;
        e1.write_bits(&mut buffer, &mut bit_offset);
        
        let mut bit_offset = 0;
        let parsed = TestEnum::parse_bits(&buffer, &mut bit_offset).unwrap();
        assert_eq!(parsed, TestEnum::field1);
        
        let mut buffer2 = Vec::new();
        let mut bit_offset2 = 0;
        let e2 = TestEnum::field2(0); // it initialized with T::Default in parsing but we explicitly set 0 here
        e2.write_bits(&mut buffer2, &mut bit_offset2);
        
        let mut bit_offset2 = 0;
        let parsed2 = TestEnum::parse_bits(&buffer2, &mut bit_offset2).unwrap();
        assert_eq!(parsed2, TestEnum::field2(0)); // it returns T::Default which is 0 for u8
        
        let mut buffer3 = Vec::new();
        let mut bit_offset3 = 0;
        let e3 = TestEnum::field3(1000); // 1000 < 1524
        e3.write_bits(&mut buffer3, &mut bit_offset3);
        
        let mut bit_offset3 = 0;
        let parsed3 = TestEnum::parse_bits(&buffer3, &mut bit_offset3).unwrap();
        assert_eq!(parsed3, TestEnum::field3(1000));
        
        let mut buffer4 = Vec::new();
        let mut bit_offset4 = 0;
        let e4 = TestEnum::field4(2000); // >= 1524, so it falls to catch-all
        e4.write_bits(&mut buffer4, &mut bit_offset4);
        
        let mut bit_offset4 = 0;
        let parsed4 = TestEnum::parse_bits(&buffer4, &mut bit_offset4).unwrap();
        assert_eq!(parsed4, TestEnum::field4(2000));
        
        // Test Vec<u8> as repr_type
        let data6 = vec![1, 2, 3, 4];
        let mut bit_offset6 = 0;
        let parsed6 = EthIpType::parse_bits(&data6, &mut bit_offset6).unwrap();
        assert_eq!(parsed6, EthIpType::IPv4(vec![1, 2, 3, 4]));
        
        let data7 = vec![1, 2, 3, 4, 5, 6];
        let mut bit_offset7 = 0;
        let parsed7 = EthIpType::parse_bits(&data7, &mut bit_offset7).unwrap();
        assert_eq!(parsed7, EthIpType::IPv6(vec![1, 2, 3, 4, 5, 6]));
    }
}pub mod tests_usize;
pub mod tests_peek;
