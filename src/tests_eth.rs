use crate::*;
use alloc::vec::Vec;

make_struct!{ ethII_frame{
    mac_dest: u64,
    mac_src: u64,
    if peek(16) == 0x8100 {
        vlan: u16,
    }
}}
#[test]
fn test_eth() {}
