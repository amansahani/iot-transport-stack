use super::checksum::ipv4_pseudo_header_checksum;
use std::fmt;

/// RFC 768 User Datagram Protocol (UDP)
/// Header Format:
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |          Source Port          |       Destination Port        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |            Length             |           Checksum            |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpPacket {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub payload: Vec<u8>,
}

impl UdpPacket {
    pub const HEADER_LEN: usize = 8;

    pub fn new(src_port: u16, dst_port: u16, payload: Vec<u8>) -> Self {
        let length = (Self::HEADER_LEN + payload.len()) as u16;
        Self {
            src_port,
            dst_port,
            length,
            checksum: 0,
            payload,
        }
    }

    /// Calculate RFC 768 checksum using IPv4 Pseudo-Header
    pub fn compute_checksum(&mut self, src_ip: [u8; 4], dst_ip: [u8; 4]) {
        self.checksum = 0;
        let mut pseudo_sum = ipv4_pseudo_header_checksum(src_ip, dst_ip, 17, self.length);

        // Serialize packet without checksum
        let mut packet_bytes = Vec::with_capacity(self.length as usize);
        packet_bytes.extend_from_slice(&self.src_port.to_be_bytes());
        packet_bytes.extend_from_slice(&self.dst_port.to_be_bytes());
        packet_bytes.extend_from_slice(&self.length.to_be_bytes());
        packet_bytes.extend_from_slice(&0u16.to_be_bytes());
        packet_bytes.extend_from_slice(&self.payload);

        // Fold pseudo_sum into packet bytes checksum
        let mut i = 0;
        while i + 1 < packet_bytes.len() {
            let word = u16::from_be_bytes([packet_bytes[i], packet_bytes[i + 1]]);
            pseudo_sum = pseudo_sum.wrapping_add(word as u32);
            i += 2;
        }
        if i < packet_bytes.len() {
            let word = u16::from_be_bytes([packet_bytes[i], 0]);
            pseudo_sum = pseudo_sum.wrapping_add(word as u32);
        }

        while (pseudo_sum >> 16) > 0 {
            pseudo_sum = (pseudo_sum & 0xFFFF) + (pseudo_sum >> 16);
        }

        let computed = !pseudo_sum as u16;
        // RFC 768: If the computed checksum is zero, it is transmitted as all ones (0xFFFF).
        self.checksum = if computed == 0 { 0xFFFF } else { computed };
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::HEADER_LEN + self.payload.len());
        buf.extend_from_slice(&self.src_port.to_be_bytes());
        buf.extend_from_slice(&self.dst_port.to_be_bytes());
        buf.extend_from_slice(&self.length.to_be_bytes());
        buf.extend_from_slice(&self.checksum.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < Self::HEADER_LEN {
            return Err("Packet too short for UDP header");
        }
        let src_port = u16::from_be_bytes([buf[0], buf[1]]);
        let dst_port = u16::from_be_bytes([buf[2], buf[3]]);
        let length = u16::from_be_bytes([buf[4], buf[5]]);
        let checksum = u16::from_be_bytes([buf[6], buf[7]]);

        if (length as usize) < Self::HEADER_LEN || buf.len() < (length as usize) {
            return Err("Invalid UDP length field");
        }

        let payload = buf[Self::HEADER_LEN..length as usize].to_vec();

        Ok(Self {
            src_port,
            dst_port,
            length,
            checksum,
            payload,
        })
    }
}

impl fmt::Display for UdpPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UDP Port {} -> {} | Len: {} bytes | Checksum: 0x{:04X}",
            self.src_port, self.dst_port, self.length, self.checksum
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_udp_roundtrip() {
        let payload = b"IoT Temperature Sensor Data: 24.5C".to_vec();
        let mut packet = UdpPacket::new(5000, 8080, payload.clone());
        packet.compute_checksum([192, 168, 1, 10], [192, 168, 1, 1]);
        
        let bytes = packet.serialize();
        let parsed = UdpPacket::parse(&bytes).unwrap();
        
        assert_eq!(parsed.src_port, 5000);
        assert_eq!(parsed.dst_port, 8080);
        assert_eq!(parsed.payload, payload);
        assert_eq!(parsed.checksum, packet.checksum);
    }
}
