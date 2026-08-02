use core::convert::{TryFrom, Into};

pub trait NetworkParse {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str>
    where
        Self: Sized;
    fn write_bits(&self, buffer: &mut Vec<u8>, bit_offset: &mut usize);
}

impl NetworkParse for usize {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str> {
        Ok(0)
    }
    fn write_bits(&self, buffer: &mut Vec<u8>, bit_offset: &mut usize) {}
}
