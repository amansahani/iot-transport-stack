use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Header {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags_fragment_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
}

impl Ipv4Header {
    pub const PROTOCOL_TCP: u8 = 6;
    pub const PROTOCOL_UDP: u8 = 17;

    pub fn new(src_ip: [u8; 4], dst_ip: [u8; 4], protocol: u8, payload_len: u16) -> Self {
        let total_length = 20 + payload_len;
        let mut hdr = Self {
            version_ihl: 0x45, // IPv4, Header Length = 5 (20 bytes)
            dscp_ecn: 0,
            total_length,
            identification: 0x1234,
            flags_fragment_offset: 0x4000, // Don't fragment
            ttl: 64,
            protocol,
            checksum: 0,
            src_ip,
            dst_ip,
        };
        hdr.checksum = hdr.calculate_header_checksum();
        hdr
    }

    pub fn calculate_header_checksum(&self) -> u16 {
        let mut bytes = [0u8; 20];
        self.serialize_into(&mut bytes);
        bytes[10] = 0;
        bytes[11] = 0;
        super::checksum::calculate_checksum(&bytes)
    }

    pub fn serialize_into(&self, buf: &mut [u8]) {
        buf[0] = self.version_ihl;
        buf[1] = self.dscp_ecn;
        buf[2..4].copy_from_slice(&self.total_length.to_be_bytes());
        buf[4..6].copy_from_slice(&self.identification.to_be_bytes());
        buf[6..8].copy_from_slice(&self.flags_fragment_offset.to_be_bytes());
        buf[8] = self.ttl;
        buf[9] = self.protocol;
        buf[10..12].copy_from_slice(&self.checksum.to_be_bytes());
        buf[12..16].copy_from_slice(&self.src_ip);
        buf[16..20].copy_from_slice(&self.dst_ip);
    }

    pub fn parse(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < 20 {
            return Err("IPv4 buffer too short");
        }
        let version_ihl = buf[0];
        if (version_ihl >> 4) != 4 {
            return Err("Not an IPv4 packet");
        }
        let total_length = u16::from_be_bytes([buf[2], buf[3]]);
        let identification = u16::from_be_bytes([buf[4], buf[5]]);
        let flags_fragment_offset = u16::from_be_bytes([buf[6], buf[7]]);
        let ttl = buf[8];
        let protocol = buf[9];
        let checksum = u16::from_be_bytes([buf[10], buf[11]]);
        let mut src_ip = [0u8; 4];
        let mut dst_ip = [0u8; 4];
        src_ip.copy_from_slice(&buf[12..16]);
        dst_ip.copy_from_slice(&buf[16..20]);

        Ok(Self {
            version_ihl,
            dscp_ecn: buf[1],
            total_length,
            identification,
            flags_fragment_offset,
            ttl,
            protocol,
            checksum,
            src_ip,
            dst_ip,
        })
    }
}

impl fmt::Display for Ipv4Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IPv4 {}.{}.{}.{} -> {}.{}.{}.{} [Proto: {}]",
            self.src_ip[0], self.src_ip[1], self.src_ip[2], self.src_ip[3],
            self.dst_ip[0], self.dst_ip[1], self.dst_ip[2], self.dst_ip[3],
            if self.protocol == Self::PROTOCOL_TCP { "TCP" } else if self.protocol == Self::PROTOCOL_UDP { "UDP" } else { "Other" }
        )
    }
}
