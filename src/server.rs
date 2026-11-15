use std::io::{Read, Write};
use std::net::{TcpListener, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::tcp::{TcpConnection, TcpState};
use crate::proto::TcpSegment;

/// A TransportServer that listens for incoming real network connections
/// via `std::net::TcpListener`, and runs each session through our custom RFC TCP TCB.
/// Supports plugging in ANY Application Layer protocol handler (HTTP, MQTT, Custom IoT Telemetry, etc.).
pub struct TransportServer {
    listener: TcpListener,
    running: Arc<AtomicBool>,
}

impl TransportServer {
    /// Binds to a local socket address (e.g., "127.0.0.1:8080").
    pub fn bind<A: ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Stops the server listener loop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Handles an incoming connection with a modular application-level protocol callback.
    /// The handler receives:
    /// - `app_payload: &[u8]`
    /// - Returns: `Option<Vec<u8>>` response payload to send back.
    pub fn accept_one<F>(&self, timeout: Duration, mut handler: F) -> std::io::Result<bool>
    where
        F: FnMut(&[u8]) -> Option<Vec<u8>>,
    {
        let start = std::time::Instant::now();
        loop {
            if !self.running.load(Ordering::SeqCst) || start.elapsed() > timeout {
                return Ok(false);
            }

            match self.listener.accept() {
                Ok((mut stream, _peer_addr)) => {
                    stream.set_nodelay(true)?;
                    stream.set_read_timeout(Some(Duration::from_millis(500)))?;

                    let mut tcb = TcpConnection::new([127, 0, 0, 1], 8080, [127, 0, 0, 1], 5000, 5000);
                    tcb.state = TcpState::Listen;

                    let mut buf = [0u8; 4096];
                    let mut stream_active = true;

                    // Process stream
                    while stream_active {
                        match stream.read(&mut buf) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                let mut offset = 0;
                                while offset < n {
                                    if let Ok(seg) = TcpSegment::parse(&buf[offset..n]) {
                                        offset += (seg.data_offset as usize * 4) + seg.payload.len();

                                        let has_payload = !seg.payload.is_empty();
                                        let app_reply = if has_payload {
                                            handler(&seg.payload)
                                        } else {
                                            None
                                        };

                                        // 1. TCB handles incoming segment
                                        let maybe_reply = tcb.handle_incoming(&seg);
                                        if let Some(reply) = maybe_reply {
                                            let _ = stream.write_all(&reply.serialize());
                                        }

                                        // 2. Send application reply if generated
                                        if let Some(resp_payload) = app_reply {
                                            let data_seg = tcb.build_data_segment(resp_payload);
                                            let _ = stream.write_all(&data_seg.serialize());
                                        }

                                        if seg.flags.fin() {
                                            stream_active = false;
                                            break;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                                // Wait a bit more before disconnecting
                                std::thread::sleep(Duration::from_millis(50));
                            }
                            Err(_) => break,
                        }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    return Ok(true);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(e),
            }
        }
    }
}
