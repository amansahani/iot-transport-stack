use std::collections::BTreeMap;

/// TCP Sliding Window Send and Receive Buffers
#[derive(Debug)]
pub struct SendBuffer {
    /// In-flight unacknowledged segments: seq -> (data, retransmit_count, sent_instant)
    pub unacked: BTreeMap<u32, Vec<u8>>,
    /// Unsent data queued by the application
    pub unsent: Vec<u8>,
}

impl SendBuffer {
    pub fn new() -> Self {
        Self {
            unacked: BTreeMap::new(),
            unsent: Vec::new(),
        }
    }

    pub fn push_app_data(&mut self, data: &[u8]) {
        self.unsent.extend_from_slice(data);
    }

    pub fn mark_sent(&mut self, seq: u32, data: Vec<u8>) {
        self.unacked.insert(seq, data);
    }

    pub fn acknowledge_up_to(&mut self, ack_seq: u32) {
        // Remove all segments where seq + len <= ack_seq
        let keys_to_remove: Vec<u32> = self.unacked
            .iter()
            .filter_map(|(&seq, data)| {
                let end = seq.wrapping_add(data.len() as u32);
                if end <= ack_seq {
                    Some(seq)
                } else {
                    None
                }
            })
            .collect();

        for k in keys_to_remove {
            self.unacked.remove(&k);
        }
    }
}

#[derive(Debug)]
pub struct RecvBuffer {
    /// Out of order segments received: seq -> data
    pub out_of_order: BTreeMap<u32, Vec<u8>>,
    /// Assembled in-order stream ready for application read
    pub stream: Vec<u8>,
}

impl RecvBuffer {
    pub fn new() -> Self {
        Self {
            out_of_order: BTreeMap::new(),
            stream: Vec::new(),
        }
    }

    pub fn insert_segment(&mut self, rcv_nxt: &mut u32, seq: u32, data: Vec<u8>) -> bool {
        if seq == *rcv_nxt {
            *rcv_nxt = rcv_nxt.wrapping_add(data.len() as u32);
            self.stream.extend_from_slice(&data);

            // Check if subsequent buffered segments can now be stitched in
            while let Some((&next_seq, _)) = self.out_of_order.iter().next() {
                if next_seq == *rcv_nxt {
                    let next_data = self.out_of_order.remove(&next_seq).unwrap();
                    *rcv_nxt = rcv_nxt.wrapping_add(next_data.len() as u32);
                    self.stream.extend_from_slice(&next_data);
                } else if next_seq < *rcv_nxt {
                    self.out_of_order.remove(&next_seq);
                } else {
                    break;
                }
            }
            true
        } else if seq > *rcv_nxt {
            // Out-of-order segment stored for future stitching (SACK benefit)
            self.out_of_order.insert(seq, data);
            false
        } else {
            // Duplicate segment already delivered
            false
        }
    }

    pub fn get_sack_blocks(&self) -> Vec<(u32, u32)> {
        let mut blocks = Vec::new();
        for (&seq, data) in &self.out_of_order {
            let right_edge = seq.wrapping_add(data.len() as u32);
            blocks.push((seq, right_edge));
            if blocks.len() == 4 { // Max 4 SACK blocks (RFC 2018)
                break;
            }
        }
        blocks
    }
}
