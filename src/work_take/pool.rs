use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::super::super::{ReceiverWaitStrategy, ShareStrategy};
use super::channel::make_channels;
use super::shared::Data as SharedData;
use super::worker::Worker;
use super::worker::Config as WorkerConfig;

pub struct Pool {
    local_worker: Worker,
    handles: Vec<thread::JoinHandle<()>>,
}

impl Pool {
    pub fn new(num_threads: usize,
               capacity: usize,
               timeout: Duration) -> Pool {

    }
}
