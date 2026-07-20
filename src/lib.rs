#![cfg_attr(not(feature="std"), no_std)]
use core::convert::{TryFrom, From, Into};
extern crate alloc;
use alloc::vec::Vec;

pub trait NetworkParse: Sized + TryFrom<Vec<u8>> + From<*mut u8> + Into<Vec<u8>> {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str>;
    fn parse_bits_ptr(ptr: *const u8, bit_offset: &mut usize) -> Self;
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
            0b1110_0000, 0b1100_0000, // byte 16-17: field7 = -500 (i12) -> 1110_0000_1100
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
    fn test_from_ptr() {
        let mut data: Vec<u8> = vec![
            0b1110_0000, // byte 0: field1 = 7 (u3) = 111, consume(5) = 00000
            0b0001_1000, // byte 1: field2 = 3 (u5) = 00011, consume(3) = 000
            0x01, 0x02, 0x03, // byte 2-4: field3 (Vec<u8> of length field2 = 3)
            b't', b'e', b's', b't', 0x00, // byte 5-9: field4 (CStr)
            0x04, 0x00, // byte 10-11: field5 (u16) = 1024 (0x0400)
            10, 20, 30, 40, // byte 12-15: field6 ([u8; 4])
            0b1110_0000, 0b1100_0000, // byte 16-17: field7 = -500 (i12) -> 1110_0000_1100
        ];
        
        let ptr = data.as_mut_ptr();
        let parsed = TestStruct::from(ptr);
        
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
            0b1110_0000, 0b1100_0000, // byte 16-17: field7 = -500 (i12) -> 1110_0000_1100
        ];
        
        assert_eq!(data, expected_data);
    }
}