use core::convert::{TryFrom, Into};

pub trait NetworkParse: Sized + TryFrom<Vec<u8>> + Into<Vec<u8>> {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str>;
    fn write_bits(self, buffer: &mut Vec<u8>, bit_offset: &mut usize);
}

impl NetworkParse for usize {
    fn parse_bits(data: &[u8], bit_offset: &mut usize) -> Result<Self, &'static str> {
        Ok(0)
    }
    fn write_bits(self, buffer: &mut Vec<u8>, bit_offset: &mut usize) {}
}
