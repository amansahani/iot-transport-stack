use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use crate::tcp::TcpConnection;
use crate::proto::TcpSegment;

/// A TransportClient that connects using a real OS TCP stream,
/// running every segment through our custom RFC TCP TCB (State Machine,
/// Sliding Window, and Jacobson/Karn RTT estimator).
pub struct TransportClient {
    stream: TcpStream,
    pub tcb: TcpConnection,
    raw_buffer: Vec<u8>,
}

impl TransportClient {
    /// Connects to a remote server, sets socket options, and establishes the custom TCB state.
    pub fn connect<A: ToSocketAddrs>(addr: A, local_port: u16, remote_port: u16) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;

        let mut client = Self {
            stream,
            tcb: TcpConnection::new([127, 0, 0, 1], local_port, [127, 0, 0, 1], remote_port, 1000),
            raw_buffer: Vec::new(),
        };

        // Active open handshake
        let syn = client.tcb.build_syn();
        let wire_bytes = syn.serialize();
        client.stream.write_all(&wire_bytes)?;

        // Wait for SYN-ACK
        let mut in_buf = [0u8; 2048];
        let n = client.stream.read(&mut in_buf)?;
        if let Ok(seg) = TcpSegment::parse(&in_buf[..n]) {
            if let Some(ack) = client.tcb.handle_incoming(&seg) {
                client.stream.write_all(&ack.serialize())?;
            }
        }

        Ok(client)
    }

    /// Sends application layer data (HTTP, MQTT, CoAP, etc.) framed inside our custom TCP segments.
    pub fn send_app_data(&mut self, payload: &[u8]) -> std::io::Result<()> {
        let seg = self.tcb.build_data_segment(payload.to_vec());
        let wire_bytes = seg.serialize();
        self.stream.write_all(&wire_bytes)
    }

    /// Reads application payload delivered by the server through the TCP stream.
    pub fn read_app_data(&mut self) -> std::io::Result<Vec<u8>> {
        let mut buf = [0u8; 4096];
        let start = std::time::Instant::now();

        while start.elapsed() < Duration::from_millis(1500) {
            match self.stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    self.raw_buffer.extend_from_slice(&buf[..n]);
                    while !self.raw_buffer.is_empty() {
                        if let Ok(seg) = TcpSegment::parse(&self.raw_buffer) {
                            let consumed = (seg.data_offset as usize * 4) + seg.payload.len();
                            self.raw_buffer.drain(..consumed.min(self.raw_buffer.len()));

                            if let Some(ack) = self.tcb.handle_incoming(&seg) {
                                let _ = self.stream.write_all(&ack.serialize());
                            }

                            if !seg.payload.is_empty() {
                                return Ok(seg.payload);
                            }
                        } else {
                            break;
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(e),
            }
        }

        Ok(Vec::new())
    }

    /// Graceful teardown using custom FIN / ACK segments.
    pub fn close(&mut self) -> std::io::Result<()> {
        let fin = self.tcb.build_fin();
        let _ = self.stream.write_all(&fin.serialize());
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        Ok(())
    }
}
