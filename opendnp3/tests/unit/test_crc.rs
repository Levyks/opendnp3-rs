//! 1:1 port of `TestCRC.cpp` (1 test case).

use opendnp3::link::crc;
use opendnp3::util::hex::from_hex;

#[test]
fn crc_test() {
    let hs = from_hex("05 64 05 C0 01 00 00 04 E9 21").unwrap();
    assert_eq!(hs.len(), 10);
    assert_eq!(crc::calc_crc(&hs[..8]), 0x21E9);
}
