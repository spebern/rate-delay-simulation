use crate::channel::{Packet, Receiver, Response, Sender};
use async_std::task;
use chrono;
use std::collections::HashMap;
use std::time;

pub struct BaseStation {
    responses: Sender<Response>,
    packets: Receiver<Packet>,
    processing_interval: time::Duration,
    cached_packets: HashMap<u32, Packet>,
    processed_packets: Vec<Packet>,
}

impl BaseStation {
    pub fn new(
        processing_rate: usize,
        packets: Receiver<Packet>,
        responses: Sender<Response>,
        num_packets: u32,
    ) -> Self {
        let processing_interval =
            time::Duration::from_micros((1.0 / processing_rate as f64 * 1_000_000.0) as u64);
        Self {
            packets,
            responses,
            processing_interval,
            cached_packets: HashMap::new(),
            processed_packets: Vec::with_capacity(num_packets as usize),
        }
    }

    pub async fn run_simulation(mut self) -> Vec<Packet> {
        while let Some(mut packet) = self.packets.recv().await {
            task::sleep(self.processing_interval).await;
            self.responses.send(Response::ACK(packet.id)).await;

            let mut next_packet_id = self.processed_packets.len() as u32;

            // are we missing some packets?
            if packet.id > next_packet_id {
                // this is a valid packet so we cache it so that we can use it later
                self.cached_packets.insert(packet.id, packet.clone());
            } else if packet.id == next_packet_id {
                // process this packet
                packet.processed = Some(chrono::Utc::now());
                self.processed_packets.push(packet);
                next_packet_id += 1;

                // process all following cached packets
                for i in next_packet_id.. {
                    if let Some(mut packet) = self.cached_packets.remove(&i) {
                        packet.processed = Some(chrono::Utc::now());
                        self.processed_packets.push(packet);
                    } else {
                        break;
                    }
                }
                if self.processed_packets.capacity() == self.processed_packets.len() {
                    return self.processed_packets;
                }
            }
        }
        self.processed_packets
    }
}
