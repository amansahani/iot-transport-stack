use std::fmt;

/// RFC 9293 (replaces RFC 793) Control Bits / Flags
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct TcpFlags(pub u8);

impl TcpFlags {
    pub const FIN: u8 = 0b0000_0001; // 0x01
    pub const SYN: u8 = 0b0000_0010; // 0x02
    pub const RST: u8 = 0b0000_0100; // 0x04
    pub const PSH: u8 = 0b0000_1000; // 0x08
    pub const ACK: u8 = 0b0001_0000; // 0x10
    pub const URG: u8 = 0b0010_0000; // 0x20
    pub const ECE: u8 = 0b0100_0000; // 0x40 (RFC 3168 ECN Echo)
    pub const CWR: u8 = 0b1000_0000; // 0x80 (RFC 3168 Congestion Window Reduced)

    pub fn new() -> Self {
        Self(0)
    }

    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn contains(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    pub fn syn(&self) -> bool { self.contains(Self::SYN) }
    pub fn ack(&self) -> bool { self.contains(Self::ACK) }
    pub fn fin(&self) -> bool { self.contains(Self::FIN) }
    pub fn rst(&self) -> bool { self.contains(Self::RST) }
    pub fn psh(&self) -> bool { self.contains(Self::PSH) }
    pub fn urg(&self) -> bool { self.contains(Self::URG) }
    pub fn ece(&self) -> bool { self.contains(Self::ECE) }
    pub fn cwr(&self) -> bool { self.contains(Self::CWR) }
}

impl fmt::Debug for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.syn() { flags.push("SYN"); }
        if self.ack() { flags.push("ACK"); }
        if self.fin() { flags.push("FIN"); }
        if self.rst() { flags.push("RST"); }
        if self.psh() { flags.push("PSH"); }
        if self.urg() { flags.push("URG"); }
        if self.ece() { flags.push("ECE"); }
        if self.cwr() { flags.push("CWR"); }

        if flags.is_empty() {
            write!(f, "[NONE]")
        } else {
            write!(f, "[{}]", flags.join("|"))
        }
    }
}

impl fmt::Display for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
