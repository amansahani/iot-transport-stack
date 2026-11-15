use std::sync::atomic::{AtomicUsize, Ordering};

/// Standard Ethernet Maximum Transmission Unit (MTU) buffer size.
pub const PACKET_BUFFER_SIZE: usize = 2048;

/// Descriptor representing an allocated packet chunk within an aligned UMEM block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketDescriptor {
    pub chunk_index: usize,
    pub offset: usize,
    pub length: usize,
}

/// Aligned User-Space Memory Pool (UMEM) mimicking AF_XDP / DPDK chunk regions.
/// Pre-allocates a contiguous block of memory with zero runtime heap allocations (`malloc`/`free`)
/// on the packet hot path.
pub struct UmemPool {
    buffer: Vec<u8>,
    num_chunks: usize,
    chunk_size: usize,
    free_stack: Vec<usize>,
    alloc_count: AtomicUsize,
}

impl UmemPool {
    /// Initializes a contiguous UMEM area divided into equal-sized chunks.
    pub fn new(num_chunks: usize, chunk_size: usize) -> Self {
        let total_size = num_chunks * chunk_size;
        // Allocate contiguous memory once up-front (simulating mmap / DPDK hugepages)
        let mut buffer = Vec::with_capacity(total_size);
        buffer.resize(total_size, 0);

        let mut free_stack = Vec::with_capacity(num_chunks);
        for i in (0..num_chunks).rev() {
            free_stack.push(i);
        }

        Self {
            buffer,
            num_chunks,
            chunk_size,
            free_stack,
            alloc_count: AtomicUsize::new(0),
        }
    }

    /// Allocates a chunk descriptor from the pre-allocated pool (O(1), zero syscalls).
    pub fn alloc(&mut self) -> Option<PacketDescriptor> {
        self.free_stack.pop().map(|idx| {
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
            PacketDescriptor {
                chunk_index: idx,
                offset: idx * self.chunk_size,
                length: 0,
            }
        })
    }

    /// Returns a chunk back to the free pool (O(1)).
    pub fn free(&mut self, desc: PacketDescriptor) {
        if desc.chunk_index < self.num_chunks {
            self.free_stack.push(desc.chunk_index);
            self.alloc_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Get mutable slice view of the packet buffer without copying.
    pub fn get_slice_mut(&mut self, desc: &PacketDescriptor) -> &mut [u8] {
        let start = desc.offset;
        let end = start + self.chunk_size;
        &mut self.buffer[start..end]
    }

    /// Get immutable slice view of the valid packet bytes.
    pub fn get_slice(&self, desc: &PacketDescriptor) -> &[u8] {
        let start = desc.offset;
        let end = start + desc.length;
        &self.buffer[start..end]
    }

    pub fn total_chunks(&self) -> usize {
        self.num_chunks
    }

    pub fn free_chunks(&self) -> usize {
        self.free_stack.len()
    }
}
