/// RFC 5681 TCP Congestion Control
/// Implements: Slow Start, Congestion Avoidance, Fast Retransmit, and Fast Recovery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionPhase {
    SlowStart,
    CongestionAvoidance,
    FastRecovery,
}

#[derive(Debug, Clone)]
pub struct CongestionControl {
    /// Congestion Window in bytes (cwnd)
    pub cwnd: usize,
    /// Slow Start Threshold in bytes (ssthresh)
    pub ssthresh: usize,
    /// Maximum Segment Size in bytes
    pub smss: usize,
    /// Current phase
    pub phase: CongestionPhase,
    /// Duplicate ACK counter (for 3-duplicate ACK Fast Retransmit trigger)
    pub dup_ack_count: u32,
    /// Last acknowledged sequence number
    pub last_ack: u32,
}

impl CongestionControl {
    pub fn new(smss: usize) -> Self {
        // RFC 5681 initial cwnd: 2 to 4 segments (typically 2 * SMSS or 3 * SMSS)
        let initial_cwnd = 2 * smss;
        Self {
            cwnd: initial_cwnd,
            ssthresh: 65535, // High initial threshold
            smss,
            phase: CongestionPhase::SlowStart,
            dup_ack_count: 0,
            last_ack: 0,
        }
    }

    /// Called upon receipt of a new valid ACK
    pub fn on_ack_received(&mut self, ack_num: u32) {
        if ack_num == self.last_ack {
            self.dup_ack_count += 1;
            if self.dup_ack_count == 3 {
                // RFC 5681 Fast Retransmit trigger!
                self.ssthresh = (self.cwnd / 2).max(2 * self.smss);
                self.cwnd = self.ssthresh + (3 * self.smss);
                self.phase = CongestionPhase::FastRecovery;
            } else if self.dup_ack_count > 3 && self.phase == CongestionPhase::FastRecovery {
                // In Fast Recovery, inflate cwnd by SMSS for each additional dup ACK
                self.cwnd += self.smss;
            }
        } else if ack_num > self.last_ack {
            // New ACK received!
            if self.phase == CongestionPhase::FastRecovery {
                // Deflate cwnd back to ssthresh and exit Fast Recovery
                self.cwnd = self.ssthresh;
                self.phase = CongestionPhase::CongestionAvoidance;
            } else {
                match self.phase {
                    CongestionPhase::SlowStart => {
                        // In Slow Start: cwnd increases by SMSS for each ACK (exponential growth)
                        self.cwnd += self.smss;
                        if self.cwnd >= self.ssthresh {
                            self.phase = CongestionPhase::CongestionAvoidance;
                        }
                    }
                    CongestionPhase::CongestionAvoidance => {
                        // In Congestion Avoidance: cwnd += SMSS * SMSS / cwnd per ACK (linear growth approx 1 SMSS per RTT)
                        let increment = ((self.smss * self.smss) / self.cwnd).max(1);
                        self.cwnd += increment;
                    }
                    _ => {}
                }
            }
            self.last_ack = ack_num;
            self.dup_ack_count = 0;
        }
    }

    /// Called on RTO expiration (retransmission timeout)
    pub fn on_timeout(&mut self) {
        // RFC 5681: ssthresh = max(FlightSize/2, 2*SMSS), cwnd = 1 * SMSS (or initial cwnd)
        self.ssthresh = (self.cwnd / 2).max(2 * self.smss);
        self.cwnd = self.smss;
        self.phase = CongestionPhase::SlowStart;
        self.dup_ack_count = 0;
    }
}
