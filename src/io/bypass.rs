use crate::io::mempool::{UmemPool, PacketDescriptor, PACKET_BUFFER_SIZE};
use crate::io::ring::CircularRing;

/// AF_XDP / DPDK Kernel-Bypass Queue Pair Architecture.
/// Manages Direct Memory Access (DMA) between the hardware NIC driver and user space.
/// Comprises 4 zero-copy rings:
/// 1. Fill Ring: User space provides free buffer addresses to NIC DMA.
/// 2. Rx Ring: NIC informs user space of newly received packets.
/// 3. Tx Ring: User space provides packets ready for NIC line transmission.
/// 4. Completion Ring: NIC signals packet transmission has completed so buffers can be recycled.
pub struct KernelBypassQueuePair {
    pub umem: UmemPool,
    pub fill_ring: CircularRing,
    pub rx_ring: CircularRing,
    pub tx_ring: CircularRing,
    pub comp_ring: CircularRing,

    // Performance telemetry
    pub rx_packets: usize,
    pub tx_packets: usize,
    pub dropped_packets: usize,
}

impl KernelBypassQueuePair {
    /// Creates a kernel-bypass queue pair with a pre-allocated UMEM region.
    pub fn new(num_chunks: usize, ring_capacity: usize) -> Self {
        let mut umem = UmemPool::new(num_chunks, PACKET_BUFFER_SIZE);
        let mut fill_ring = CircularRing::new(ring_capacity);
        let rx_ring = CircularRing::new(ring_capacity);
        let tx_ring = CircularRing::new(ring_capacity);
        let comp_ring = CircularRing::new(ring_capacity);

        // Pre-populate the Fill Ring with initial free descriptors for RX DMA
        for _ in 0..ring_capacity.min(num_chunks / 2) {
            if let Some(desc) = umem.alloc() {
                let _ = fill_ring.enqueue(desc);
            }
        }

        Self {
            umem,
            fill_ring,
            rx_ring,
            tx_ring,
            comp_ring,
            rx_packets: 0,
            tx_packets: 0,
            dropped_packets: 0,
        }
    }

    /// Simulates hardware NIC RX Direct Memory Access (DMA):
    /// Bypasses kernel sk_buff overhead; writes directly into pre-allocated UMEM buffer.
    pub fn simulate_nic_rx_dma(&mut self, wire_bytes: &[u8]) -> Result<(), &'static str> {
        // 1. Pop a descriptor from the Fill Ring that user-space made available
        let mut desc = match self.fill_ring.dequeue() {
            Some(d) => d,
            None => {
                // If fill ring exhausted, allocate directly from UMEM pool
                match self.umem.alloc() {
                    Some(d) => d,
                    None => {
                        self.dropped_packets += 1;
                        return Err("RX dropped: No free UMEM buffers (Zero-Copy Ring Exhaustion)");
                    }
                }
            }
        };

        // 2. Direct memory write into user-space UMEM (Zero kernel-to-user copy)
        let slice = self.umem.get_slice_mut(&desc);
        let copy_len = wire_bytes.len().min(slice.len());
        slice[..copy_len].copy_from_slice(&wire_bytes[..copy_len]);
        desc.length = copy_len;

        // 3. Post descriptor to the RX Ring for worker thread processing
        if self.rx_ring.enqueue(desc).is_err() {
            self.umem.free(desc);
            self.dropped_packets += 1;
            return Err("RX Ring Full");
        }

        self.rx_packets += 1;
        Ok(())
    }

    /// User-space worker thread polls RX Ring (Poll Mode Driver - PMD).
    /// Zero context switches, zero system calls (`read()` avoided).
    pub fn poll_rx(&mut self) -> Option<PacketDescriptor> {
        self.rx_ring.dequeue()
    }

    /// User-space worker thread submits a packet to the TX Ring.
    pub fn transmit(&mut self, desc: PacketDescriptor) -> Result<(), &'static str> {
        self.tx_ring.enqueue(desc)?;
        self.tx_packets += 1;
        Ok(())
    }

    /// Simulates NIC line transmission & buffer recycling:
    /// NIC drains the TX ring and puts descriptor into the Completion Ring.
    pub fn flush_tx_to_wire(&mut self) -> Vec<Vec<u8>> {
        let mut wire_frames = Vec::new();

        while let Some(tx_desc) = self.tx_ring.dequeue() {
            let data = self.umem.get_slice(&tx_desc).to_vec();
            wire_frames.push(data);

            // Notify completion ring so user-space can recycle the descriptor
            let _ = self.comp_ring.enqueue(tx_desc);
        }

        wire_frames
    }

    /// Recycles finished TX packets back into the UMEM free pool.
    pub fn recycle_tx_completions(&mut self) {
        while let Some(desc) = self.comp_ring.dequeue() {
            self.umem.free(desc);
        }
    }

    /// Replenishes the Fill Ring with free buffers for subsequent incoming frames.
    pub fn refill_rx_buffers(&mut self) {
        while self.fill_ring.len() < self.fill_ring.is_empty() as usize {
            if let Some(desc) = self.umem.alloc() {
                if self.fill_ring.enqueue(desc).is_err() {
                    self.umem.free(desc);
                    break;
                }
            } else {
                break;
            }
        }
    }
}
