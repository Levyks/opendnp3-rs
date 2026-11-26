//! The DNP3 CRC-16 (CRC-16/DNP: reflected polynomial 0xA6BC, init 0x0000,
//! final XOR 0xFFFF).

const fn make_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u16;
        let mut j = 0;
        while j < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA6BC
            } else {
                crc >> 1
            };
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

const CRC_TABLE: [u16; 256] = make_table();

/// Calculate the DNP3 CRC over `input`.
pub fn calc_crc(input: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in input {
        let index = ((crc ^ byte as u16) & 0xFF) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    !crc
}

/// Append the 2-byte little-endian CRC of `buf[..len]` immediately after it.
/// `buf` must have at least `len + 2` bytes.
pub fn add_crc(buf: &mut [u8], len: usize) {
    let crc = calc_crc(&buf[..len]);
    buf[len..len + 2].copy_from_slice(&crc.to_le_bytes());
}

/// The CRC bytes (little-endian) for `input`.
pub fn crc_bytes(input: &[u8]) -> [u8; 2] {
    calc_crc(input).to_le_bytes()
}

/// Verify that `input[..len]` is followed by its correct 2-byte CRC.
pub fn is_correct_crc(input: &[u8], len: usize) -> bool {
    if input.len() < len + 2 {
        return false;
    }
    let expected = u16::from_le_bytes([input[len], input[len + 1]]);
    calc_crc(&input[..len]) == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_reference_entries() {
        assert_eq!(CRC_TABLE[0], 0x0000);
        assert_eq!(CRC_TABLE[1], 0x365E);
        assert_eq!(CRC_TABLE[2], 0x6CBC);
        assert_eq!(CRC_TABLE[255], 0x1235);
    }

    #[test]
    fn check_value() {
        // CRC-16/DNP check value for ASCII "123456789" is 0xEA82
        assert_eq!(calc_crc(b"123456789"), 0xEA82);
    }
}
