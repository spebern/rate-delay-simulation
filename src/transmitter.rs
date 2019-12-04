use crate::channel::{Packet, Receiver, Response, Sender};
use async_std::{
    sync::{self, Arc, Mutex},
    task,
};
use chrono;
use std::collections::HashSet;
use std::time;

pub struct Transmitter {
    transmission_interval: time::Duration,
    unconfirmed_packets_tx: sync::Sender<(time::Instant, Packet)>,
    tokens: sync::Receiver<()>,
    packets: Sender<Packet>,
    retransmission_delay: time::Duration,
}

impl Transmitter {
    pub async fn new(
        transmission_rate: usize,
        packets: Sender<Packet>,
        responses: Receiver<Response>,
        retransmission_delay: time::Duration,
        max_unconfirmed_packets: usize,
    ) -> Self {
        let send_interval =
            time::Duration::from_micros((1.0 / transmission_rate as f64 * 1_000_000.0) as u64);

        let (unconfirmed_packets_tx, unconfimred_packets_rx) = sync::channel(1000);
        let (tokens_tx, tokens_rx) = sync::channel(max_unconfirmed_packets);
        for _ in 0..max_unconfirmed_packets {
            tokens_tx.send(()).await;
        }

        let confirmed_packets = Arc::new(Mutex::new(HashSet::new()));

        task::spawn(Transmitter::handle_responses(
            responses,
            confirmed_packets.clone(),
            tokens_tx,
        ));
        task::spawn(Transmitter::handle_resubmissions(
            packets.clone(),
            unconfirmed_packets_tx.clone(),
            unconfimred_packets_rx,
            retransmission_delay,
            confirmed_packets,
        ));

        Self {
            transmission_interval: send_interval,
            packets,
            unconfirmed_packets_tx,
            retransmission_delay,
            tokens: tokens_rx,
        }
    }

    async fn handle_responses(
        responses: Receiver<Response>,
        confirmed_packets: Arc<Mutex<HashSet<u32>>>,
        tokens: sync::Sender<()>,
    ) {
        while let Some(res) = responses.recv().await {
            match res {
                Response::ACK(packet_id) => {
                    tokens.send(()).await;
                    confirmed_packets.lock().await.insert(packet_id);
                }
                Response::NACK(_) => unreachable!(),
            }
        }
    }

    async fn handle_resubmissions(
        packets: Sender<Packet>,
        unconfirmed_packets_tx: sync::Sender<(time::Instant, Packet)>,
        unconfirmed_packets_rx: sync::Receiver<(time::Instant, Packet)>,
        delay: time::Duration,
        confirmed_packets: Arc<Mutex<HashSet<u32>>>,
    ) {
        while let Some((due, packet)) = unconfirmed_packets_rx.recv().await {
            if confirmed_packets.lock().await.get(&packet.id).is_some() {
                continue;
            }
            let now = time::Instant::now();
            if due > now {
                task::sleep(due - now).await;
                if confirmed_packets.lock().await.get(&packet.id).is_some() {
                    continue;
                }
            }
            packets.send(packet.clone()).await;
            unconfirmed_packets_tx
                .send((time::Instant::now() + delay, packet))
                .await;
        }
    }

    pub async fn run_simulation(self, num_packets: u32) {
        if num_packets == 0 {
            return;
        }

        for packet_id in 0..num_packets {
            task::sleep(self.transmission_interval).await;

            // assure that we don't have too many outstanding packets
            self.tokens.recv().await;

            let packet = Packet {
                id: packet_id,
                transmitted: chrono::Utc::now(),
                processed: None,
            };
            self.packets.send(packet.clone()).await;
            self.unconfirmed_packets_tx
                .send((time::Instant::now() + self.retransmission_delay, packet))
                .await;
        }
    }
}
