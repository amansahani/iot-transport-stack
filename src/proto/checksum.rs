/// RFC 1071 / RFC 768 / RFC 9293 Internet Checksum Implementation
/// One's complement sum of 16-bit words.
pub fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]);
        sum = sum.wrapping_add(word as u32);
        i += 2;
    }

    // If odd length, pad with zero on the right
    if i < data.len() {
        let word = u16::from_be_bytes([data[i], 0]);
        sum = sum.wrapping_add(word as u32);
    }

    // Fold 32-bit sum to 16 bits
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

/// Computes pseudo-header checksum contribution for IPv4
pub fn ipv4_pseudo_header_checksum(src_ip: [u8; 4], dst_ip: [u8; 4], protocol: u8, length: u16) -> u32 {
    let mut sum: u32 = 0;
    sum += u16::from_be_bytes([src_ip[0], src_ip[1]]) as u32;
    sum += u16::from_be_bytes([src_ip[2], src_ip[3]]) as u32;
    sum += u16::from_be_bytes([dst_ip[0], dst_ip[1]]) as u32;
    sum += u16::from_be_bytes([dst_ip[2], dst_ip[3]]) as u32;
    sum += protocol as u32;
    sum += length as u32;
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_even() {
        let data = [0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06];
        let csum = calculate_checksum(&data);
        assert_ne!(csum, 0);
    }
}
