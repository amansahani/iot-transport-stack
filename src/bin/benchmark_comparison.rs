use colored::*;
use iot_transport_stack::io::KernelBypassQueuePair;
use iot_transport_stack::proto::UdpPacket;
use std::time::Instant;

fn main() {
    println!("{}", "================================================================================".cyan());
    println!("{}", "   KERNEL-BYPASS I/O vs STANDARD OS SOCKET BENCHMARK".bold().yellow());
    println!("{}", "   Direct Memory Access (DMA), Pre-allocated UMEM Pool & Circular Lock-free Rings".bold().green());
    println!("{}", "================================================================================\n".cyan());

    let iterations = 200_000;
    let payload = b"GET /sensor/temperature HTTP/1.1\r\nHost: node.local\r\n\r\n";
    let mut udp = UdpPacket::new(5000, 8080, payload.to_vec());
    udp.compute_checksum([192, 168, 1, 10], [192, 168, 1, 1]);
    let wire_frame = udp.serialize();

    println!(">>> Test Payload: UDP Segment carrying HTTP GET ({} bytes)", wire_frame.len());
    println!(">>> Running {} packet iterations on the hot path...\n", iterations);

    // -------------------------------------------------------------
    // Test 1: Standard OS Socket Paradigm (Dynamic Alloc + Kernel Boundary Copy)
    // -------------------------------------------------------------
    let start_std = Instant::now();
    let mut std_received_bytes = 0usize;
    for _ in 0..iterations {
        // Simulates kernel networking stack per-packet overhead:
        // 1. sk_buff heap allocation
        let mut kernel_sk_buff = Vec::with_capacity(wire_frame.len());
        kernel_sk_buff.extend_from_slice(&wire_frame); // Copy 1: NIC DMA into kernel buffer

        // 2. Syscall context switch overhead simulation (simulating read() syscall)
        std::hint::black_box(&kernel_sk_buff);

        // 3. User-space buffer allocation (malloc) & copy from kernel space
        let mut user_space_buffer = vec![0u8; kernel_sk_buff.len()];
        user_space_buffer.copy_from_slice(&kernel_sk_buff); // Copy 2: Kernel to User memory
        std_received_bytes = std_received_bytes.wrapping_add(user_space_buffer.len());
    }
    let duration_std = start_std.elapsed();
    let pps_std = (iterations as f64) / duration_std.as_secs_f64();
    let mpps_std = pps_std / 1_000_000.0;
    let latency_std_ns = (duration_std.as_nanos() as f64) / (iterations as f64);

    // -------------------------------------------------------------
    // Test 2: Kernel-Bypass Direct DMA Paradigm (UMEM + Ring)
    // -------------------------------------------------------------
    let mut qp = KernelBypassQueuePair::new(512, 256);

    let start_bypass = Instant::now();
    let mut bypass_received_bytes = 0usize;
    for _ in 0..iterations {
        // 1. Direct hardware NIC DMA into pre-allocated user-space UMEM (Zero copy)
        qp.simulate_nic_rx_dma(&wire_frame).expect("DMA to UMEM");

        // 2. Poll Mode Driver (PMD) consumes packet descriptor from RX ring (Zero syscall)
        if let Some(desc) = qp.poll_rx() {
            let slice = qp.umem.get_slice(&desc);
            std::hint::black_box(slice);
            bypass_received_bytes = bypass_received_bytes.wrapping_add(slice.len());

            // 3. Recycle descriptor back to fill ring for next incoming DMA
            qp.umem.free(desc);
            qp.refill_rx_buffers();
        }
    }
    let duration_bypass = start_bypass.elapsed();
    let pps_bypass = (iterations as f64) / duration_bypass.as_secs_f64();
    let mpps_bypass = pps_bypass / 1_000_000.0;
    let latency_bypass_ns = (duration_bypass.as_nanos() as f64) / (iterations as f64);

    // Prevent compiler from optimizing away the test loops
    std::hint::black_box(std_received_bytes);
    std::hint::black_box(bypass_received_bytes);

    // -------------------------------------------------------------
    // Results & Comparative Analysis
    // -------------------------------------------------------------
    println!("{}", "--- [PARADIGM 1]: Standard OS Socket (Syscalls + Double-Copy Heap Buffers) ---".yellow());
    println!("  • Total Time:      {:.2?}", duration_std);
    println!("  • Throughput:      {:.2} Mpps ({:.0} packets/sec)", mpps_std, pps_std);
    println!("  • Per-Packet Cost: {:.1} ns / packet", latency_std_ns);
    println!("  • Memory Copies:   2 copies (NIC -> sk_buff -> User Buffer) + 1 Heap alloc/free per packet");

    println!("\n{}", "--- [PARADIGM 2]: Kernel-Bypass (AF_XDP / DPDK Style DMA + UMEM Ring Buffers) ---".green());
    println!("  • Total Time:      {:.2?}", duration_bypass);
    println!("  • Throughput:      {:.2} Mpps ({:.0} packets/sec)", mpps_bypass, pps_bypass);
    println!("  • Per-Packet Cost: {:.1} ns / packet", latency_bypass_ns);
    println!("  • Memory Copies:   0 copies (Direct NIC DMA into User-Space UMEM Pool)");
    println!("  • Syscalls:        0 syscalls (Poll Mode Driver on Lock-Free Circular Rings)");

    println!("\n{}", "================================================================================".cyan());
    println!("  {} Kernel-Bypass operates with ZERO allocations on the hot path (deterministic latency)!",
        "ARCHITECTURAL CONCLUSION:".bold().bright_green());
    println!("{}", "================================================================================\n".cyan());
}
