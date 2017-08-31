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
               task_capacity: usize,
               share_strategy: ShareStrategy,
               wait_strategy: ReceiverWaitStrategy,
               receiver_timeout: Duration) -> Pool {
        assert!(num_threads > 0);

        let mut channels = make_channels(num_threads);
        let shared_data = Arc::new(SharedData::new(num_threads-1));

        let local_worker = Worker::new(WorkerConfig {
            index: 0,
            shared_data: shared_data.clone(),
            task_capacity: task_capacity,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            receiver_timeout: receiver_timeout,
            channel_data: channels.remove(0),
        });
        
        let handles = (1..num_threads).into_iter().zip(channels.into_iter())
            .map(|(index, channel)| {
                let worker = Worker::new(WorkerConfig {
                    index: index,
                    shared_data: shared_data.clone(),
                    task_capacity: task_capacity,
                    share_strategy: share_strategy,
                    wait_strategy: wait_strategy,
                    receiver_timeout: receiver_timeout,
                    channel_data: channel,
                });

                thread::spawn(move || {
                    worker.run();
                })
            }).collect::<Vec<_>>();

        Pool {
            local_worker: local_worker,
            handles: handles,
        }
    }

    pub fn run(&mut self) {
        self.local_worker.run();
    }

    pub fn run_once(&mut self) {
        self.local_worker.run_once();
    }
}
