use crate::io::mempool::PacketDescriptor;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Power-of-two Circular Lock-Free Ring Buffer (modeling DPDK `rte_ring` / Linux `XSK_RING`).
/// Eliminates locking contention on hot paths between network drivers and worker threads.
pub struct CircularRing {
    entries: Vec<Option<PacketDescriptor>>,
    capacity: usize,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl CircularRing {
    /// Creates a circular ring with power-of-two capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0 && (capacity & (capacity - 1)) == 0, "Capacity must be a power of two");
        let mut entries = Vec::with_capacity(capacity);
        entries.resize_with(capacity, || None);

        Self {
            entries,
            capacity,
            mask: capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Enqueue a single packet descriptor without allocations.
    pub fn enqueue(&mut self, desc: PacketDescriptor) -> Result<(), &'static str> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        if head.wrapping_sub(tail) >= self.capacity {
            return Err("Ring buffer full");
        }

        let idx = head & self.mask;
        self.entries[idx] = Some(desc);
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Dequeue a packet descriptor from the ring.
    pub fn dequeue(&mut self) -> Option<PacketDescriptor> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail == head {
            return None; // Ring empty
        }

        let idx = tail & self.mask;
        let desc = self.entries[idx].take();
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        desc
    }

    /// Enqueue a batch of descriptors in a single operation (amortizing overhead).
    pub fn enqueue_burst(&mut self, descs: &[PacketDescriptor]) -> usize {
        let mut count = 0;
        for &d in descs {
            if self.enqueue(d).is_ok() {
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Dequeue a burst of up to `max` descriptors.
    pub fn dequeue_burst(&mut self, out: &mut Vec<PacketDescriptor>, max: usize) -> usize {
        let mut count = 0;
        while count < max {
            if let Some(d) = self.dequeue() {
                out.push(d);
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
