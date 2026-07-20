fn main() {
    let mut bit_offset = 0;
    let data: &[u8] = &[1, 2, 3];
    
    macro_rules! peek {
        ($bits:expr) => {{
            let mut temp_offset = bit_offset;
            temp_offset += $bits;
            temp_offset
        }}
    }
    
    if peek!(16) == 16 {
        bit_offset += 1;
    }
    println!("{}", bit_offset);
}
