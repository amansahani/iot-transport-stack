# iot-transport-stack

A modular, RFC-compliant TCP and UDP transport layer implementation written in Rust with wireless channel fading simulation and application-layer bindings (HTTP/1.1 and CoAP).

## Overview

Traditional transport protocols make assumptions based on wired network behavior. Specifically, standard TCP assumes that packet loss indicates network buffer congestion. In wireless IoT environments (e.g., IEEE 802.15.4, LoRa, cellular IoT), packet loss is predominantly caused by multipath fading, shadow fading, and interference.

This project implements:
1. **RFC 768 (UDP):** Datagram framing, IPv4 pseudo-header checksum generation and verification (RFC 1071).
2. **RFC 9293 (TCP):** Full 11-state Finite State Machine (FSM), 3-way handshake, graceful teardown, sequence/ack tracking, sliding window buffers, and TCP options parsing (MSS, Window Scale, SACK Permitted).
3. **RFC 5681 & RFC 6298:** Congestion control (Slow Start, Congestion Avoidance, Fast Retransmit) and RTT estimation with Karn's exponential backoff.
4. **RFC 9112 (HTTP/1.1) & RFC 7252 (CoAP):** Application layer payloads transmitted over TCP and UDP.
5. **Wireless Channel Simulator:** Configurable link model evaluating transport performance under packet loss (Rayleigh fading drops), jitter, latency, and bit error rate (BER).

## Standards Implemented

- **RFC 768:** User Datagram Protocol
- **RFC 1071:** Computing the Internet Checksum
- **RFC 9293:** Transmission Control Protocol (replaces RFC 793)
- **RFC 2018:** TCP Selective Acknowledgment Options (SACK)
- **RFC 5681:** TCP Congestion Control
- **RFC 6298:** Computing TCP's Retransmission Timer
- **RFC 7252:** The Constrained Application Protocol (CoAP)
- **RFC 9112:** HTTP/1.1 Message Syntax and Routing

## Architecture

```
src/
├── proto/
│   ├── checksum.rs      # RFC 1071 one's complement Internet checksum
│   ├── ipv4.rs          # IPv4 header and pseudo-header
│   ├── udp.rs           # RFC 768 datagram encoding/decoding
│   ├── tcp_flags.rs     # TCP control bits
│   ├── tcp_options.rs   # MSS, Window Scale, SACK parsing/serialization
│   ├── tcp.rs           # RFC 9293 TCP segment parser and serializer
│   └── http.rs          # HTTP/1.1 and CoAP message formats
├── tcp/
│   ├── state.rs         # 11-state FSM definitions
│   ├── rtt.rs           # RFC 6298 RTT estimator & Karn's algorithm
│   ├── congestion.rs    # RFC 5681 Slow Start & Congestion Avoidance
│   ├── buffer.rs        # Sliding window send/receive ring buffers
│   └── connection.rs    # Transmission Control Block (TCB) implementation
├── wireless/
│   └── channel.rs       # Stochastic wireless link channel simulator
└── main.rs              # Protocol validation and demonstration runner
```

## Building and Running

### Prerequisites

- Rust 1.75+ (Cargo)

### Run Unit Tests

```bash
cargo test
```

### Run Demonstration

```bash
cargo run
```

The runner executes four stages:
1. **RFC Datagram & Segment Verification:** Validates packet bitfields and checksums.
2. **TCP 3-Way Handshake:** Follows client/server state transitions through `SYN-SENT`, `SYN-RECEIVED`, and `ESTABLISHED`.
3. **HTTP/1.1 REST Transmission:** Sends an HTTP POST telemetry request with JSON payload over custom TCP segments and processes the HTTP 200 OK response.
4. **Wireless Channel Fading Evaluation:** Evaluates UDP/CoAP datagram loss against TCP/HTTP reliability and congestion backoff under a 25% wireless drop channel.

## License

MIT OR Apache-2.0
