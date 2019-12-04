use crate::path_loss::GilbertElliot;
use async_std::{sync, task};
use std::time;

#[derive(Clone)]
pub struct Packet {
    pub id: u32,
    pub transmitted: chrono::DateTime<chrono::Utc>,
    pub processed: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug)]
pub enum Response {
    ACK(u32),
    NACK(u32),
}

pub struct Receiver<T: 'static + Send>(sync::Receiver<T>);

impl<T: 'static + Send> Receiver<T> {
    pub async fn recv(&self) -> Option<T> {
        self.0.recv().await
    }
}

#[derive(Clone)]
pub struct Sender<T: 'static + Send> {
    input: sync::Sender<T>,
    path_loss_model: GilbertElliot,
    delay: time::Duration,
}

impl<T: 'static + Send> Sender<T> {
    fn new(input: sync::Sender<T>, path_loss_model: GilbertElliot, delay: time::Duration) -> Self {
        Self {
            input,
            path_loss_model,
            delay,
        }
    }

    pub async fn send(&self, msg: T) {
        if self.path_loss_model.transmit().await {
            let input = self.input.clone();
            let delay = self.delay;
            task::spawn(async move {
                task::sleep(delay).await;
                input.send(msg).await;
            });
        }
    }
}

pub fn channel<T: 'static + Send>(
    path_loss_model: GilbertElliot,
    delay: time::Duration,
) -> (Receiver<T>, Sender<T>) {
    let (input, output) = sync::channel(1000);
    (Receiver(output), Sender::new(input, path_loss_model, delay))
}
