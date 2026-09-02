#![cfg_attr(not(feature="std"), no_std)]
pub extern crate alloc;
pub use alloc::vec::Vec;
pub use alloc::boxed::Box;
pub use alloc::ffi::CString;

use core::convert::{TryFrom, Into};
extern crate self as network_parser_rs;

pub trait NetworkParse<'a>: Sized + TryFrom<alloc::vec::Vec<u8>> + TryFrom<&'a[u8]> + Into<alloc::vec::Vec<u8>> {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> core::result::Result<Self, &'static str>;
    fn write_bits(&self, buffer: &mut alloc::vec::Vec<u8>, bit_offset: &mut usize);
    
    #[inline]
    fn parse_len(data: &'a [u8]) -> core::result::Result<(Self, usize), &'static str> {
        let mut bit_offset = 0;
        let res = Self::parse_bits(data, &mut bit_offset)?;
        Ok((res, (bit_offset + 7) / 8))
    }
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
        instance.write_bits(&mut buffer, &mut bit_offset);
        
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
        instance.write_bits(&mut buffer, &mut bit_offset);
        
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

    make_struct! {
        #[derive(Debug, PartialEq)]
        ExprVecTest {
            ihl: u8,
            options: Vec<u8>; (ihl as usize * 4) - 20,
            payload: Vec<u8>,
        }
    }

    #[test]
    fn test_vec_expression_size() {
        // ihl = 6 => options length = 6 * 4 - 20 = 4 bytes
        let mut data = vec![6]; // ihl = 6
        data.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]); // options (4 bytes)
        data.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // payload (3 bytes)

        let mut bit_offset = 0;
        let packet = ExprVecTest::parse_bits(&data, &mut bit_offset).unwrap();
        assert_eq!(packet.ihl, 6);
        assert_eq!(packet.options, vec![0x11, 0x22, 0x33, 0x44]);
        assert_eq!(packet.payload, vec![0xAA, 0xBB, 0xCC]);

        let serialized: Vec<u8> = packet.into();
        assert_eq!(serialized, data);
    }

    #[test]
    fn test_write_bits_by_ref() {
        let instance = BoxTest {
            boxed_field: alloc::boxed::Box::new(BoxTestInner { inner_field: 99 }),
        };
        let mut buffer1 = Vec::new();
        let mut offset1 = 0;
        instance.write_bits(&mut buffer1, &mut offset1);

        // Can serialize multiple times without clone or move
        let mut buffer2 = Vec::new();
        let mut offset2 = 0;
        instance.write_bits(&mut buffer2, &mut offset2);
        assert_eq!(buffer1, buffer2);

        let vec_from_ref: Vec<u8> = Vec::from(&instance);
        assert_eq!(vec_from_ref, buffer1);
    }

    mod no_imports_test {
        use crate::make_struct;

        make_struct! {
            icmp_packet {
                aa: u8,
            }
        }

        make_struct! {
            #[derive(Default, Debug)]
            pub ipv4_packet {
                /// Header version
                pub header: u4,
                /// Header length
                pub ihl: u4, // len = ihl*4
                pub dscp: u6;
                pub ecn: u2;
                pub total_length: u16,
                pub identification: u16,
                pub flags: u3,
                pub frag_offset: u13,
                pub ttl: u8,
                pub protocol: u8,
                pub checksum: u16,
                pub source_ip: u32,
                pub destination_ip: u32,
                pub options: Vec<u8>; (ihl as usize * 4) - 20,
                pub payload: Vec<u8>,
                exclude {
                    pub l4_data: Option<Box<u32>>;
                };
            }
        }
    }
}
pub mod tests_usize;
pub mod tests_peek;
pub mod tests_peek_multi;
pub mod tests_eth;
