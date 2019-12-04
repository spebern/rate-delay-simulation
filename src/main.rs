mod base_station;
mod channel;
mod path_loss;
mod transmitter;

use crate::base_station::BaseStation;
use crate::channel::{channel, Packet};
use crate::path_loss::GilbertElliot;
use crate::transmitter::Transmitter;
use async_std::task;
use csv;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    /// Number of packets to send during simulation
    num_packets: u32,

    /// Probability to switch from good state to bad state
    prob_good_to_bad: f64,

    /// Probability to switch from bad state to good state
    prob_bad_to_good: f64,

    /// Error rate in good state
    err_rate_good: f64,

    /// Error rate in bad state
    err_rate_bad: f64,

    /// Delay of the channel in ms
    delay: u64,

    /// Transmission rate of transmitter in Hz
    transmission_rate: usize,

    /// Process rate of base station in Hz
    processing_rate: usize,
}

#[derive(Serialize)]
struct PacketRow {
    id: u32,
    transmitted_nanos: i64,
    processed_nanos: i64,
}

fn write_statistics(opt: Opt, packets: Vec<Packet>) -> io::Result<()> {
    let file_name = format!(
        "{}_{}_{}_{}_{}_{}_{}_{}.csv",
        opt.num_packets,
        opt.prob_bad_to_good,
        opt.prob_bad_to_good,
        opt.err_rate_good,
        opt.err_rate_bad,
        opt.delay,
        opt.transmission_rate,
        opt.processing_rate,
    );
    let dir = PathBuf::new().join("simulation-results");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = dir.join(file_name);
    let mut wtr = csv::Writer::from_path(path)?;

    for packet in packets {
        let packet_row = PacketRow {
            id: packet.id,
            transmitted_nanos: packet.transmitted.timestamp_nanos(),
            processed_nanos: packet.processed.unwrap().timestamp_nanos(),
        };
        wtr.serialize(&packet_row)?;
    }
    wtr.flush()?;
    Ok(())
}

#[async_std::main]
async fn main() -> io::Result<()> {
    let opt = Opt::from_args();

    let path_loss_model = GilbertElliot::new(
        opt.prob_good_to_bad,
        opt.prob_bad_to_good,
        opt.err_rate_good,
        opt.err_rate_bad,
    );

    let (packets_rx, packets_tx) = channel(
        path_loss_model.clone(),
        time::Duration::from_millis(opt.delay),
    );

    let path_loss_model = GilbertElliot::new(
        opt.prob_good_to_bad,
        opt.prob_bad_to_good,
        opt.err_rate_good,
        opt.err_rate_bad,
    );
    let (failed_packets_rx, failed_packets_tx) =
        channel(path_loss_model, time::Duration::from_millis(opt.delay));

    let base_station = BaseStation::new(
        opt.processing_rate,
        packets_rx,
        failed_packets_tx,
        opt.num_packets,
    );

    let transmitter = Transmitter::new(
        opt.transmission_rate,
        packets_tx,
        failed_packets_rx,
        time::Duration::from_millis(30),
        1000,
    )
    .await;

    task::spawn(transmitter.run_simulation(opt.num_packets));

    let processed_packets = base_station.run_simulation().await;

    write_statistics(opt, processed_packets)
}
