use rand::Rng;
use std::time::Duration;
use std::thread;

/// Wireless Channel Model (Simulating Path Loss, Multipath Fading, Interference & Jitter)
/// Evaluates TCP vs UDP under typical IoT links (LoRa, IEEE 802.15.4 / ZigBee, NB-IoT)
#[derive(Debug, Clone)]
pub struct WirelessChannel {
    /// Packet error rate / drop probability (0.0 to 1.0)
    pub packet_loss_rate: f64,
    /// Mean latency in milliseconds
    pub base_latency_ms: u64,
    /// Jitter in milliseconds (+/- variation)
    pub jitter_ms: u64,
    /// Bit error rate (causes checksum corruption)
    pub bit_error_rate: f64,
}

impl WirelessChannel {
    pub fn new(packet_loss_rate: f64, base_latency_ms: u64, jitter_ms: u64, bit_error_rate: f64) -> Self {
        Self {
            packet_loss_rate,
            base_latency_ms,
            jitter_ms,
            bit_error_rate,
        }
    }

    /// Pre-configured profile: Low-Power Wireless IoT (IEEE 802.15.4 / 6LoWPAN)
    pub fn zigbee_802154() -> Self {
        Self {
            packet_loss_rate: 0.12, // 12% loss due to interference in 2.4 GHz ISM band
            base_latency_ms: 25,
            jitter_ms: 10,
            bit_error_rate: 0.001,
        }
    }

    /// Pre-configured profile: Long Range Lossy IoT (LoRaWAN / NB-IoT Fading)
    pub fn lora_fading() -> Self {
        Self {
            packet_loss_rate: 0.25, // 25% deep Rayleigh fading packet drops
            base_latency_ms: 150,
            jitter_ms: 50,
            bit_error_rate: 0.005,
        }
    }

    /// Transmit raw frame through the channel: may drop, delay, or corrupt
    pub fn transmit(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let mut rng = rand::thread_rng();

        // 1. Packet Loss Check (Fading / Collision)
        if rng.gen_bool(self.packet_loss_rate) {
            return None; // Dropped over wireless link
        }

        // 2. Latency + Jitter Delay
        let jitter = if self.jitter_ms > 0 {
            rng.gen_range(0..=(self.jitter_ms * 2)) as i64 - (self.jitter_ms as i64)
        } else {
            0
        };
        let delay_ms = ((self.base_latency_ms as i64) + jitter).max(1) as u64;
        thread::sleep(Duration::from_millis(delay_ms));

        // 3. Bit Error Corruption Check
        let mut output = packet.to_vec();
        if rng.gen_bool(self.bit_error_rate) && !output.is_empty() {
            let flip_idx = rng.gen_range(0..output.len());
            output[flip_idx] ^= 0x01; // Invert one bit (will fail RFC checksum)
        }

        Some(output)
    }
}
