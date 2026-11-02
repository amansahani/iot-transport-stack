use std::time::Duration;

/// RFC 6298 Computing TCP's Retransmission Timer (RTT and RTO)
/// Also implements Karn's Algorithm (ignoring RTT for retransmitted segments)
#[derive(Debug, Clone)]
pub struct RttEstimator {
    /// Smoothed Round Trip Time (SRTT) in milliseconds
    pub srtt_ms: f64,
    /// Round Trip Time Variation (RTTVAR) in milliseconds
    pub rttvar_ms: f64,
    /// Retransmission Timeout (RTO) in milliseconds
    pub rto_ms: f64,
    /// Minimum RTO (RFC 6298 recommends 1 second, but for IoT/simulations often lower, e.g. 200ms)
    pub min_rto_ms: f64,
    /// Maximum RTO (RFC 6298 recommends at least 60 seconds)
    pub max_rto_ms: f64,
    /// Whether first measurement has been taken
    first_measurement: bool,
}

impl RttEstimator {
    pub fn new() -> Self {
        Self {
            srtt_ms: 0.0,
            rttvar_ms: 0.0,
            rto_ms: 1000.0, // Default initial RTO = 1.0 second (RFC 6298 Sec 2.1)
            min_rto_ms: 200.0,
            max_rto_ms: 60000.0,
            first_measurement: true,
        }
    }

    /// Update RTT estimates when a valid ACK is received for an un-retransmitted segment (RFC 6298 Sec 2.2 & 2.3)
    pub fn update_rtt(&mut self, sample_rtt: Duration) {
        let r = sample_rtt.as_secs_f64() * 1000.0;

        if self.first_measurement {
            // First RTT measurement (RFC 6298 Sec 2.2):
            // SRTT = R
            // RTTVAR = R / 2
            // RTO = SRTT + max(G, K * RTTVAR) where K = 4
            self.srtt_ms = r;
            self.rttvar_ms = r / 2.0;
            self.rto_ms = self.srtt_ms + (4.0 * self.rttvar_ms);
            self.first_measurement = false;
        } else {
            // Subsequent RTT measurements (RFC 6298 Sec 2.3):
            // RTTVAR = (1 - beta) * RTTVAR + beta * |SRTT - R|   where beta = 1/4 = 0.25
            // SRTT = (1 - alpha) * SRTT + alpha * R              where alpha = 1/8 = 0.125
            let alpha = 0.125;
            let beta = 0.25;

            self.rttvar_ms = (1.0 - beta) * self.rttvar_ms + beta * (self.srtt_ms - r).abs();
            self.srtt_ms = (1.0 - alpha) * self.srtt_ms + alpha * r;
            self.rto_ms = self.srtt_ms + (4.0 * self.rttvar_ms);
        }

        // Bound RTO between min_rto and max_rto
        if self.rto_ms < self.min_rto_ms {
            self.rto_ms = self.min_rto_ms;
        } else if self.rto_ms > self.max_rto_ms {
            self.rto_ms = self.max_rto_ms;
        }
    }

    /// Exponential backoff on timeout (Karn's algorithm / RFC 6298 Sec 2.5)
    pub fn on_timeout_backoff(&mut self) {
        self.rto_ms = (self.rto_ms * 2.0).min(self.max_rto_ms);
    }

    pub fn get_rto(&self) -> Duration {
        Duration::from_millis(self.rto_ms as u64)
    }
}
