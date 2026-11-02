use super::state::TcpState;
use super::rtt::RttEstimator;
use super::congestion::CongestionControl;
use super::buffer::{SendBuffer, RecvBuffer};
use crate::proto::{TcpSegment, TcpFlags, TcpOption};
use std::time::Instant;

/// RFC 9293 Transmission Control Block (TCB)
/// Maintains all state variables for a single TCP endpoint connection
pub struct TcpConnection {
    pub state: TcpState,
    pub local_ip: [u8; 4],
    pub remote_ip: [u8; 4],
    pub local_port: u16,
    pub remote_port: u16,

    // Send Sequence Variables (RFC 9293 Sec 3.3.1)
    pub snd_una: u32,  // Send unacknowledged
    pub snd_nxt: u32,  // Send next
    pub snd_wnd: u16,  // Send window
    pub snd_wl1: u32,  // Segment sequence number used for last window update
    pub snd_wl2: u32,  // Segment acknowledgment number used for last window update
    pub iss: u32,      // Initial send sequence number

    // Receive Sequence Variables (RFC 9293 Sec 3.3.1)
    pub rcv_nxt: u32,  // Receive next
    pub rcv_wnd: u16,  // Receive window
    pub irs: u32,      // Initial receive sequence number

    // RFC 6298 & RFC 5681
    pub rtt: RttEstimator,
    pub congestion: CongestionControl,
    pub send_buf: SendBuffer,
    pub recv_buf: RecvBuffer,

    // Timers & Stats
    pub last_sent_time: Option<Instant>,
    pub total_retransmissions: usize,
    pub total_bytes_sent: usize,
    pub total_bytes_recv: usize,
}

impl TcpConnection {
    pub const DEFAULT_MSS: usize = 536; // Default IPv4 RFC 879 MSS
    pub const DEFAULT_WND: u16 = 8192;

    pub fn new(local_ip: [u8; 4], local_port: u16, remote_ip: [u8; 4], remote_port: u16, iss: u32) -> Self {
        Self {
            state: TcpState::Closed,
            local_ip,
            remote_ip,
            local_port,
            remote_port,
            snd_una: iss,
            snd_nxt: iss,
            snd_wnd: Self::DEFAULT_WND,
            snd_wl1: 0,
            snd_wl2: 0,
            iss,
            rcv_nxt: 0,
            rcv_wnd: Self::DEFAULT_WND,
            irs: 0,
            rtt: RttEstimator::new(),
            congestion: CongestionControl::new(Self::DEFAULT_MSS),
            send_buf: SendBuffer::new(),
            recv_buf: RecvBuffer::new(),
            last_sent_time: None,
            total_retransmissions: 0,
            total_bytes_sent: 0,
            total_bytes_recv: 0,
        }
    }

    /// Build SYN segment to initiate connection (Active Open)
    pub fn build_syn(&mut self) -> TcpSegment {
        let mut flags = TcpFlags::new();
        flags.set(TcpFlags::SYN);

        let options = vec![
            TcpOption::MaximumSegmentSize(Self::DEFAULT_MSS as u16),
            TcpOption::SackPermitted,
        ];

        let mut seg = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.snd_nxt,
            0,
            flags,
            self.rcv_wnd,
        ).with_options(options);

        seg.compute_checksum(self.local_ip, self.remote_ip);
        self.snd_nxt = self.snd_nxt.wrapping_add(1); // SYN consumes 1 sequence number
        self.state = TcpState::SynSent;
        seg
    }

    /// Build SYN-ACK response to incoming SYN (Passive Open)
    pub fn build_syn_ack(&mut self) -> TcpSegment {
        let mut flags = TcpFlags::new();
        flags.set(TcpFlags::SYN);
        flags.set(TcpFlags::ACK);

        let options = vec![
            TcpOption::MaximumSegmentSize(Self::DEFAULT_MSS as u16),
            TcpOption::SackPermitted,
        ];

        let mut seg = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.snd_nxt,
            self.rcv_nxt,
            flags,
            self.rcv_wnd,
        ).with_options(options);

        seg.compute_checksum(self.local_ip, self.remote_ip);
        self.snd_nxt = self.snd_nxt.wrapping_add(1); // SYN consumes 1 seq num
        self.state = TcpState::SynReceived;
        seg
    }

    /// Build ACK segment
    pub fn build_ack(&mut self) -> TcpSegment {
        let mut flags = TcpFlags::new();
        flags.set(TcpFlags::ACK);

        let sack_blocks = self.recv_buf.get_sack_blocks();
        let mut options = Vec::new();
        if !sack_blocks.is_empty() {
            options.push(TcpOption::Sack(sack_blocks));
        }

        let mut seg = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.snd_nxt,
            self.rcv_nxt,
            flags,
            self.rcv_wnd,
        ).with_options(options);

        seg.compute_checksum(self.local_ip, self.remote_ip);
        seg
    }

    /// Build Data segment
    pub fn build_data_segment(&mut self, payload: Vec<u8>) -> TcpSegment {
        let mut flags = TcpFlags::new();
        flags.set(TcpFlags::ACK);
        flags.set(TcpFlags::PSH);

        let seq = self.snd_nxt;
        let len = payload.len() as u32;
        self.snd_nxt = self.snd_nxt.wrapping_add(len);
        self.total_bytes_sent += payload.len();

        let mut seg = TcpSegment::new(
            self.local_port,
            self.remote_port,
            seq,
            self.rcv_nxt,
            flags,
            self.rcv_wnd,
        ).with_payload(payload.clone());

        self.send_buf.mark_sent(seq, payload);
        self.last_sent_time = Some(Instant::now());

        seg.compute_checksum(self.local_ip, self.remote_ip);
        seg
    }

    /// Build FIN segment to initiate graceful teardown
    pub fn build_fin(&mut self) -> TcpSegment {
        let mut flags = TcpFlags::new();
        flags.set(TcpFlags::FIN);
        flags.set(TcpFlags::ACK);

        let mut seg = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.snd_nxt,
            self.rcv_nxt,
            flags,
            self.rcv_wnd,
        );

        seg.compute_checksum(self.local_ip, self.remote_ip);
        self.snd_nxt = self.snd_nxt.wrapping_add(1); // FIN consumes 1 seq num
        if self.state == TcpState::Established {
            self.state = TcpState::FinWait1;
        } else if self.state == TcpState::CloseWait {
            self.state = TcpState::LastAck;
        }
        seg
    }

    /// Handle incoming segment according to RFC 9293 state transition rules
    pub fn handle_incoming(&mut self, seg: &TcpSegment) -> Option<TcpSegment> {
        match self.state {
            TcpState::Listen => {
                if seg.flags.syn() {
                    self.irs = seg.seq_num;
                    self.rcv_nxt = seg.seq_num.wrapping_add(1);
                    return Some(self.build_syn_ack());
                }
            }
            TcpState::SynSent => {
                if seg.flags.syn() && seg.flags.ack() {
                    if seg.ack_num == self.snd_nxt {
                        self.snd_una = seg.ack_num;
                        self.irs = seg.seq_num;
                        self.rcv_nxt = seg.seq_num.wrapping_add(1);
                        self.state = TcpState::Established;
                        return Some(self.build_ack());
                    }
                }
            }
            TcpState::SynReceived => {
                if seg.flags.ack() && seg.ack_num == self.snd_nxt {
                    self.snd_una = seg.ack_num;
                    self.state = TcpState::Established;
                }
            }
            TcpState::Established => {
                let mut reply = None;

                // Handle incoming ACK
                if seg.flags.ack() {
                    if seg.ack_num > self.snd_una {
                        self.snd_una = seg.ack_num;
                        self.send_buf.acknowledge_up_to(seg.ack_num);
                        self.congestion.on_ack_received(seg.ack_num);

                        if let Some(sent) = self.last_sent_time.take() {
                            self.rtt.update_rtt(sent.elapsed());
                        }
                    } else if seg.ack_num == self.snd_una {
                        self.congestion.on_ack_received(seg.ack_num);
                    }
                }

                // Handle incoming payload
                if !seg.payload.is_empty() {
                    self.total_bytes_recv += seg.payload.len();
                    self.recv_buf.insert_segment(&mut self.rcv_nxt, seg.seq_num, seg.payload.clone());
                    reply = Some(self.build_ack());
                }

                // Handle FIN
                if seg.flags.fin() {
                    self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
                    self.state = TcpState::CloseWait;
                    return Some(self.build_ack());
                }

                return reply;
            }
            TcpState::FinWait1 => {
                if seg.flags.ack() && seg.ack_num == self.snd_nxt {
                    self.snd_una = seg.ack_num;
                    self.state = TcpState::FinWait2;
                }
                if seg.flags.fin() {
                    self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
                    self.state = TcpState::TimeWait;
                    return Some(self.build_ack());
                }
            }
            TcpState::FinWait2 => {
                if seg.flags.fin() {
                    self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
                    self.state = TcpState::TimeWait;
                    return Some(self.build_ack());
                }
            }
            TcpState::CloseWait => {
                // Application should send FIN
            }
            TcpState::LastAck => {
                if seg.flags.ack() && seg.ack_num == self.snd_nxt {
                    self.state = TcpState::Closed;
                }
            }
            TcpState::TimeWait => {
                // Remain in TimeWait, re-ack any stray FIN
                if seg.flags.fin() {
                    return Some(self.build_ack());
                }
            }
            TcpState::Closing => {
                if seg.flags.ack() && seg.ack_num == self.snd_nxt {
                    self.state = TcpState::TimeWait;
                }
            }
            TcpState::Closed => {}
        }
        None
    }
}
