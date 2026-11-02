use super::checksum::ipv4_pseudo_header_checksum;
use super::tcp_flags::TcpFlags;
use super::tcp_options::TcpOption;
use std::fmt;

/// RFC 9293 Transmission Control Protocol (TCP) Segment
///
/// Header Format:
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |          Source Port          |       Destination Port        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                        Sequence Number                        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                    Acknowledgment Number                      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |  Data |           |U|A|P|R|S|F|                               |
/// | Offset| Reserved  |R|C|S|S|Y|I|            Window             |
/// |       |           |G|K|H|T|N|N|                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |           Checksum            |         Urgent Pointer        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                    Options                    |    Padding    |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                             data                              |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpSegment {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // in 32-bit words (e.g. 5 => 20 bytes)
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
    pub options: Vec<TcpOption>,
    pub payload: Vec<u8>,
}

impl TcpSegment {
    pub const MIN_HEADER_LEN: usize = 20;

    pub fn new(src_port: u16, dst_port: u16, seq: u32, ack: u32, flags: TcpFlags, window: u16) -> Self {
        Self {
            src_port,
            dst_port,
            seq_num: seq,
            ack_num: ack,
            data_offset: 5,
            flags,
            window_size: window,
            checksum: 0,
            urgent_ptr: 0,
            options: Vec::new(),
            payload: Vec::new(),
        }
    }

    pub fn with_payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_options(mut self, options: Vec<TcpOption>) -> Self {
        self.options = options;
        // Recalculate data_offset
        let mut opt_len = 0;
        for opt in &self.options {
            opt_len += opt.serialize().len();
        }
        // Pad to multiple of 4
        let padded_opt_len = ((opt_len + 3) / 4) * 4;
        self.data_offset = (5 + (padded_opt_len / 4)) as u8;
        self
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut opt_bytes = Vec::new();
        for opt in &self.options {
            opt_bytes.extend(opt.serialize());
        }
        while opt_bytes.len() % 4 != 0 {
            opt_bytes.push(0); // RFC padding EndOfOptionList
        }

        let header_len = Self::MIN_HEADER_LEN + opt_bytes.len();
        let total_len = header_len + self.payload.len();
        let mut buf = Vec::with_capacity(total_len);

        buf.extend_from_slice(&self.src_port.to_be_bytes());
        buf.extend_from_slice(&self.dst_port.to_be_bytes());
        buf.extend_from_slice(&self.seq_num.to_be_bytes());
        buf.extend_from_slice(&self.ack_num.to_be_bytes());

        let offset_reserved_flags: u16 = ((self.data_offset as u16) << 12) | (self.flags.0 as u16);
        buf.extend_from_slice(&offset_reserved_flags.to_be_bytes());
        buf.extend_from_slice(&self.window_size.to_be_bytes());
        buf.extend_from_slice(&self.checksum.to_be_bytes());
        buf.extend_from_slice(&self.urgent_ptr.to_be_bytes());
        buf.extend_from_slice(&opt_bytes);
        buf.extend_from_slice(&self.payload);

        buf
    }

    pub fn compute_checksum(&mut self, src_ip: [u8; 4], dst_ip: [u8; 4]) {
        self.checksum = 0;
        let serialized = self.serialize();
        let tcp_length = serialized.len() as u16;

        let mut pseudo_sum = ipv4_pseudo_header_checksum(src_ip, dst_ip, 6, tcp_length);

        let mut i = 0;
        while i + 1 < serialized.len() {
            let word = u16::from_be_bytes([serialized[i], serialized[i + 1]]);
            pseudo_sum = pseudo_sum.wrapping_add(word as u32);
            i += 2;
        }
        if i < serialized.len() {
            let word = u16::from_be_bytes([serialized[i], 0]);
            pseudo_sum = pseudo_sum.wrapping_add(word as u32);
        }

        while (pseudo_sum >> 16) > 0 {
            pseudo_sum = (pseudo_sum & 0xFFFF) + (pseudo_sum >> 16);
        }

        let computed = !pseudo_sum as u16;
        self.checksum = if computed == 0 { 0xFFFF } else { computed };
    }

    pub fn parse(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < Self::MIN_HEADER_LEN {
            return Err("TCP segment too short");
        }

        let src_port = u16::from_be_bytes([buf[0], buf[1]]);
        let dst_port = u16::from_be_bytes([buf[2], buf[3]]);
        let seq_num = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let ack_num = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);

        let offset_flags = u16::from_be_bytes([buf[12], buf[13]]);
        let data_offset = ((offset_flags >> 12) & 0x0F) as u8;
        let flags = TcpFlags((offset_flags & 0x01FF) as u8);

        let window_size = u16::from_be_bytes([buf[14], buf[15]]);
        let checksum = u16::from_be_bytes([buf[16], buf[17]]);
        let urgent_ptr = u16::from_be_bytes([buf[18], buf[19]]);

        let header_len = (data_offset as usize) * 4;
        if header_len < Self::MIN_HEADER_LEN || buf.len() < header_len {
            return Err("Invalid TCP data offset");
        }

        let options_bytes = &buf[Self::MIN_HEADER_LEN..header_len];
        let mut options = TcpOption::parse_all(options_bytes).unwrap_or_default();
        // Remove padding EndOfOptionList or NoOperation from parsed options list for clean comparison
        options.retain(|opt| !matches!(opt, TcpOption::EndOfOptionList | TcpOption::NoOperation));
        let payload = buf[header_len..].to_vec();

        Ok(Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_ptr,
            options,
            payload,
        })
    }
}

impl fmt::Display for TcpSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TCP {}:{} -> {}:{} | Seq={}, Ack={}, Flags={}, Win={}, DataLen={}",
            "local", self.src_port, "remote", self.dst_port,
            self.seq_num, self.ack_num, self.flags, self.window_size, self.payload.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_roundtrip_with_options() {
        let mut flags = TcpFlags::new();
        flags.set(TcpFlags::SYN);

        let options = vec![
            TcpOption::MaximumSegmentSize(1460),
            TcpOption::WindowScale(7),
            TcpOption::SackPermitted,
        ];

        let mut seg = TcpSegment::new(12345, 80, 1000, 0, flags, 65535)
            .with_options(options);

        seg.compute_checksum([192, 168, 1, 50], [10, 0, 0, 1]);

        let raw = seg.serialize();
        let parsed = TcpSegment::parse(&raw).unwrap();

        assert_eq!(parsed.src_port, 12345);
        assert_eq!(parsed.dst_port, 80);
        assert_eq!(parsed.seq_num, 1000);
        assert!(parsed.flags.syn());
        assert_eq!(parsed.options.len(), 3);
        assert_eq!(parsed.checksum, seg.checksum);
    }
}
