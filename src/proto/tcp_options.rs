/// RFC 9293 & RFC 7323 / RFC 2018 TCP Options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcpOption {
    EndOfOptionList,                   // Kind 0
    NoOperation,                       // Kind 1 (Padding)
    MaximumSegmentSize(u16),           // Kind 2, Length 4 (RFC 793/RFC 9293)
    WindowScale(u8),                   // Kind 3, Length 3 (RFC 7323)
    SackPermitted,                     // Kind 4, Length 2 (RFC 2018)
    Sack(Vec<(u32, u32)>),             // Kind 5, Variable (RFC 2018: left/right edge pairs)
    Timestamp { val: u32, echo: u32 }, // Kind 8, Length 10 (RFC 7323)
    Unknown(u8, Vec<u8>),
}

impl TcpOption {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            TcpOption::EndOfOptionList => vec![0],
            TcpOption::NoOperation => vec![1],
            TcpOption::MaximumSegmentSize(mss) => {
                let mut v = vec![2, 4];
                v.extend_from_slice(&mss.to_be_bytes());
                v
            }
            TcpOption::WindowScale(shift) => vec![3, 3, *shift],
            TcpOption::SackPermitted => vec![4, 2],
            TcpOption::Sack(blocks) => {
                let len = 2 + (blocks.len() * 8) as u8;
                let mut v = vec![5, len];
                for (left, right) in blocks {
                    v.extend_from_slice(&left.to_be_bytes());
                    v.extend_from_slice(&right.to_be_bytes());
                }
                v
            }
            TcpOption::Timestamp { val, echo } => {
                let mut v = vec![8, 10];
                v.extend_from_slice(&val.to_be_bytes());
                v.extend_from_slice(&echo.to_be_bytes());
                v
            }
            TcpOption::Unknown(kind, data) => {
                let mut v = vec![*kind, (data.len() + 2) as u8];
                v.extend_from_slice(data);
                v
            }
        }
    }

    pub fn parse_all(mut buf: &[u8]) -> Result<Vec<Self>, &'static str> {
        let mut options = Vec::new();
        while !buf.is_empty() {
            let kind = buf[0];
            if kind == 0 {
                options.push(TcpOption::EndOfOptionList);
                break;
            }
            if kind == 1 {
                options.push(TcpOption::NoOperation);
                buf = &buf[1..];
                continue;
            }
            if buf.len() < 2 {
                return Err("Option truncated");
            }
            let len = buf[1] as usize;
            if len < 2 || buf.len() < len {
                return Err("Invalid option length");
            }

            let opt_data = &buf[2..len];
            match kind {
                2 => {
                    if opt_data.len() == 2 {
                        let mss = u16::from_be_bytes([opt_data[0], opt_data[1]]);
                        options.push(TcpOption::MaximumSegmentSize(mss));
                    }
                }
                3 => {
                    if opt_data.len() == 1 {
                        options.push(TcpOption::WindowScale(opt_data[0]));
                    }
                }
                4 => {
                    options.push(TcpOption::SackPermitted);
                }
                5 => {
                    let mut blocks = Vec::new();
                    let mut i = 0;
                    while i + 8 <= opt_data.len() {
                        let left = u32::from_be_bytes([opt_data[i], opt_data[i+1], opt_data[i+2], opt_data[i+3]]);
                        let right = u32::from_be_bytes([opt_data[i+4], opt_data[i+5], opt_data[i+6], opt_data[i+7]]);
                        blocks.push((left, right));
                        i += 8;
                    }
                    options.push(TcpOption::Sack(blocks));
                }
                8 => {
                    if opt_data.len() == 8 {
                        let val = u32::from_be_bytes([opt_data[0], opt_data[1], opt_data[2], opt_data[3]]);
                        let echo = u32::from_be_bytes([opt_data[4], opt_data[5], opt_data[6], opt_data[7]]);
                        options.push(TcpOption::Timestamp { val, echo });
                    }
                }
                other => {
                    options.push(TcpOption::Unknown(other, opt_data.to_vec()));
                }
            }
            buf = &buf[len..];
        }
        Ok(options)
    }
}
