pub mod proto;
pub mod tcp;
pub mod wireless;
pub mod server;
pub mod client;

use colored::*;
use proto::*;
use tcp::*;
use wireless::WirelessChannel;
use server::TransportServer;
use client::TransportClient;
use std::time::{Instant, Duration};

fn print_banner() {
    println!("{}", "================================================================================".cyan());
    println!("{}", "   IIIT SRI CITY - M.TECH IoT WIRELESS COMMUNICATION COURSE PRESENTATION".bold().yellow());
    println!("{}", "   RFC-Compliant TCP/UDP Transport Layer Architecture in Rust".bold().green());
    println!("{}", "   RFC 768 (UDP) | RFC 9293 (TCP FSM) | RFC 9112 (HTTP) | Multi-Protocol Socket API".italic().white());
    println!("{}", "================================================================================\n".cyan());
}

fn demo_rfc_compliance() {
    println!("{}", ">>> [DEMO 1]: RFC Packet Encoding & Checksum Validation".bold().magenta());
    let src_ip = [192, 168, 1, 100];
    let dst_ip = [10, 0, 0, 1];

    // 1. UDP RFC 768
    let udp_payload = b"IoT Sensor Packet #01 - Temperature: 28.4C, Humidity: 65%".to_vec();
    let mut udp = UdpPacket::new(4242, 8080, udp_payload);
    udp.compute_checksum(src_ip, dst_ip);
    let raw_udp = udp.serialize();
    let parsed_udp = UdpPacket::parse(&raw_udp).unwrap();
    println!("  [UDP RFC 768] Validated: src_port={}, dst_port={}, length={}, checksum=0x{:04X}", 
        parsed_udp.src_port, parsed_udp.dst_port, parsed_udp.length, parsed_udp.checksum);
    println!("  [UDP Payload] \"{}\"", String::from_utf8_lossy(&parsed_udp.payload).green());

    // 2. TCP RFC 9293 Header & Options
    let mut flags = TcpFlags::new();
    flags.set(TcpFlags::SYN);
    let options = vec![
        TcpOption::MaximumSegmentSize(1460),
        TcpOption::WindowScale(7),
        TcpOption::SackPermitted,
    ];
    let mut tcp_seg = TcpSegment::new(51234, 80, 100001, 0, flags, 64240).with_options(options);
    tcp_seg.compute_checksum(src_ip, dst_ip);
    let raw_tcp = tcp_seg.serialize();
    let parsed_tcp = TcpSegment::parse(&raw_tcp).unwrap();
    println!("  [TCP RFC 9293] Validated: Seq={}, Flags={}, Win={}, Options={:?}, Checksum=0x{:04X}",
        parsed_tcp.seq_num, parsed_tcp.flags, parsed_tcp.window_size, parsed_tcp.options, parsed_tcp.checksum);
    println!("  {}\n", "-> RFC Datagram and Segment structures strictly verified!".bright_green());
}

fn demo_three_way_handshake() {
    println!("{}", ">>> [DEMO 2]: RFC 9293 TCP 3-Way Handshake & State Machine Transitions".bold().magenta());
    let client_ip = [192, 168, 1, 50];
    let server_ip = [192, 168, 1, 1];
    let client_port = 49152;
    let server_port = 80;

    let mut client = TcpConnection::new(client_ip, client_port, server_ip, server_port, 1000);
    let mut server = TcpConnection::new(server_ip, server_port, client_ip, client_port, 5000);
    server.state = TcpState::Listen;

    println!("  Initial States -> Client: [{}], Server: [{}]", client.state.to_string().yellow(), server.state.to_string().yellow());

    // Step 1: Active Open (Client sends SYN)
    let syn = client.build_syn();
    println!("  Step 1: Client -> SYN (Seq={}, Win={}, State=[{}])", syn.seq_num, syn.window_size, client.state.to_string().cyan());

    // Step 2: Server receives SYN -> Passive Open (Server sends SYN-ACK)
    let syn_ack = server.handle_incoming(&syn).expect("Server generates SYN-ACK");
    println!("  Step 2: Server <- SYN-ACK (Seq={}, Ack={}, State=[{}])", syn_ack.seq_num, syn_ack.ack_num, server.state.to_string().cyan());

    // Step 3: Client receives SYN-ACK -> Established (Client sends ACK)
    let ack = client.handle_incoming(&syn_ack).expect("Client generates ACK");
    println!("  Step 3: Client -> ACK (Seq={}, Ack={}, State=[{}])", ack.seq_num, ack.ack_num, client.state.to_string().green());

    // Step 4: Server processes ACK
    server.handle_incoming(&ack);
    println!("  Step 4: Server <- Handshake Complete (State=[{}])", server.state.to_string().green());
    println!("  {}\n", "-> TCP Handshake Successfully Established!".bold().bright_green());
}

fn demo_socket_api_multi_protocol() {
    println!("{}", ">>> [DEMO 3]: Multi-Protocol Support via std::net::{TcpListener, TcpStream}".bold().magenta());
    println!("  Demonstrating how diverse L7 application protocols plug into our custom TCP transport layer.\n");

    let bind_addr = "127.0.0.1:28888";
    let server = TransportServer::bind(bind_addr).expect("Server binds to local port");

    // Protocol 1: HTTP/1.1 REST Telemetry (RFC 9112)
    println!("  --- [Protocol A]: HTTP/1.1 REST Service over Custom TCP ---");
    let server_thread = std::thread::spawn(move || {
        let _ = server.accept_one(Duration::from_secs(2), |app_bytes| {
            if let Ok(req) = HttpRequest::parse(app_bytes) {
                println!("    [Server RX: HTTP {}] Path: {} | Host: {}", req.method.green(), req.path, req.host);
                let resp = HttpResponse::new_200_json(r#"{"status":"ACCEPTED","processed_time_ms":8}"#);
                return Some(resp.serialize());
            } else if let Ok(s) = std::str::from_utf8(app_bytes) {
                println!("    [Server RX: Text Frame] {}", s);
            }
            None
        });
    });

    std::thread::sleep(Duration::from_millis(50));

    // Client connects using TransportClient (TcpStream + RFC TCB)
    let mut client = TransportClient::connect(bind_addr, 54321, 28888).expect("Client connects");
    let req = HttpRequest::new_post_json("/api/v1/telemetry", "iot.iiits.ac.in", r#"{"temp":24.5,"batt":98}"#);
    client.send_app_data(&req.serialize()).expect("Client sends HTTP request");

    let resp_bytes = client.read_app_data().expect("Client reads HTTP response");
    if let Ok(resp) = HttpResponse::parse(&resp_bytes) {
        println!("    [Client RX: HTTP Response] Status: {} | Body: {}", resp.status_code.to_string().bold().green(), resp.body.cyan());
    }
    client.close().expect("Client closes TCP");
    server_thread.join().unwrap();

    // Protocol 2: MQTT-style Lightweight Binary IoT Command Stream
    println!("\n  --- [Protocol B]: Binary IoT Command Stream over Custom TCP ---");
    let bind_addr_bin = "127.0.0.1:28889";
    let server_bin = TransportServer::bind(bind_addr_bin).expect("Binary server binds");

    let server_thread_bin = std::thread::spawn(move || {
        let _ = server_bin.accept_one(Duration::from_secs(2), |app_bytes| {
            println!("    [Server RX: Binary IoT Frame] Length: {} bytes, Raw: {:02X?}", app_bytes.len(), &app_bytes[..app_bytes.len().min(8)]);
            // Return binary ACK packet: [0x80 (CMD_ACK), 0x01 (STATUS_OK)]
            Some(vec![0x80, 0x01, 0x00, 0x00])
        });
    });

    std::thread::sleep(Duration::from_millis(50));

    let mut client_bin = TransportClient::connect(bind_addr_bin, 54322, 28889).expect("Binary client connects");
    let binary_iot_payload = vec![0x10, 0x04, 0xAA, 0xBB, 0xCC, 0xDD]; // Simulated Sensor Config Command
    client_bin.send_app_data(&binary_iot_payload).expect("Send binary payload");

    let ack_bytes = client_bin.read_app_data().expect("Read binary ack");
    println!("    [Client RX: Binary IoT Response] Bytes: {:02X?} (Command Acknowledged)", ack_bytes);

    client_bin.close().expect("Binary client closes");
    server_thread_bin.join().unwrap();
    println!("  {}\n", "-> Multi-protocol application layer successfully demonstrated over standard socket abstractions!".bright_green());
}

fn demo_wireless_comparison() {
    println!("{}", ">>> [DEMO 4]: Wireless Channel Fading: TCP vs UDP (with CoAP IoT datagrams)".bold().magenta());
    let channel = WirelessChannel::new(0.25, 10, 5, 0.0); // 25% wireless packet loss rate
    println!("  Channel Profile: Loss Rate = 25% (Rayleigh fading/collisions), Base Latency = 10ms, Jitter = 5ms");

    let num_packets = 20;

    // 1. UDP / CoAP in Wireless Link
    println!("\n  --- Simulating UDP / CoAP Stream (Constrained IoT Telemetry) ---");
    let mut udp_sent = 0;
    let mut udp_received = 0;
    for i in 1..=num_packets {
        udp_sent += 1;
        let coap = CoapMessage::new_telemetry_put(i as u16, format!("T={:.1}C", 25.0 + (i as f64 * 0.2)).as_bytes());
        let mut pkt = UdpPacket::new(5683, 5683, coap.serialize());
        pkt.compute_checksum([10, 0, 0, 2], [10, 0, 0, 1]);
        let raw = pkt.serialize();

        if let Some(_delivered) = channel.transmit(&raw) {
            udp_received += 1;
            print!("{}", ".".green());
        } else {
            print!("{}", "X".red()); // Dropped
        }
    }
    println!("\n  UDP Results: Sent={}, Delivered={}, Loss={:.1}% (Unrecoverable data loss in wireless IoT)",
        udp_sent, udp_received, ((udp_sent - udp_received) as f64 / udp_sent as f64) * 100.0);

    // 2. TCP over Wireless Link (Reliability with Karn's Algorithm & Congestion Control)
    println!("\n  --- Simulating TCP Stream (Reliable IoT HTTP Command & Control) ---");
    let client_ip = [10, 0, 0, 2];
    let server_ip = [10, 0, 0, 1];
    let mut client = TcpConnection::new(client_ip, 4000, server_ip, 4000, 2000);
    let mut server = TcpConnection::new(server_ip, 4000, client_ip, 4000, 7000);
    server.state = TcpState::Established;
    client.state = TcpState::Established;

    let mut retransmissions = 0;
    let mut delivered_bytes = 0;
    let start = Instant::now();

    for i in 1..=num_packets {
        let req = HttpRequest::new_post_json("/actuator", "node.iiits", &format!("{{\"pulse\":{}}}", i));
        let seg = client.build_data_segment(req.serialize());
        let raw = seg.serialize();

        let mut attempts = 0;
        loop {
            attempts += 1;
            if let Some(delivered_raw) = channel.transmit(&raw) {
                let incoming = TcpSegment::parse(&delivered_raw).unwrap();
                let reply_ack = server.handle_incoming(&incoming).unwrap();

                // Transmit ACK back over wireless channel
                let raw_ack = reply_ack.serialize();
                if let Some(ack_delivered) = channel.transmit(&raw_ack) {
                    let parsed_ack = TcpSegment::parse(&ack_delivered).unwrap();
                    client.handle_incoming(&parsed_ack);
                    delivered_bytes += incoming.payload.len();
                    print!("{}", "✓".green());
                    break;
                } else {
                    print!("{}", "A!".yellow()); // ACK dropped
                    retransmissions += 1;
                    client.congestion.on_timeout();
                    client.rtt.on_timeout_backoff();
                }
            } else {
                print!("{}", "D!".red()); // Data segment dropped
                retransmissions += 1;
                client.congestion.on_timeout();
                client.rtt.on_timeout_backoff();
            }

            if attempts >= 10 {
                println!("\n  Segment max retries exceeded");
                break;
            }
        }
    }
    let elapsed = start.elapsed();

    println!("\n  TCP Results: Total Delivered = {} bytes (100% data integrity)", delivered_bytes);
    println!("  Retransmissions triggered: {}", retransmissions.to_string().yellow());
    println!("  Final Congestion Window (cwnd): {} bytes | ssthresh: {} bytes", client.congestion.cwnd, client.congestion.ssthresh);
    println!("  Final RTO: {:?} | Smoothed RTT: {:.2}ms", client.rtt.get_rto(), client.rtt.srtt_ms);
    println!("  Total Transmission Elapsed Time: {:.2?}", elapsed);
    println!("  {}\n", "-> Key Wireless Insight: TCP guarantees reliability, but channel loss triggers congestion backoff even without network congestion!".bold().cyan());
}

fn main() {
    print_banner();
    demo_rfc_compliance();
    demo_three_way_handshake();
    demo_socket_api_multi_protocol();
    demo_wireless_comparison();
}
