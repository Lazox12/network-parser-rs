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

