use std::thread;
use std::time::Duration;

use super::{ReceiverWaitStrategy, ShareStrategy};
// use task::Task;
use super::channel::{make_receiver_initiated_channels, make_sender_initiated_channels};
use super::worker::{WorkerRI, WorkerSI};
use super::worker::ConfigRI as WorkerRIConfig;
use super::worker::ConfigSI as WorkerSIConfig;

pub struct PoolRI {
    local_worker: WorkerRI,
    handles: Vec<thread::JoinHandle<()>>,
}

impl PoolRI {
    pub fn new(num_threads:    usize,
               task_capcity:   usize,
               share_strategy: ShareStrategy,
               wait_strategy: ReceiverWaitStrategy,
               receiver_timeout: Duration,
               channel_timeout: Duration) -> PoolRI {
        assert!(num_threads > 0);

        let mut channels = make_receiver_initiated_channels(num_threads);

        let local_worker = WorkerRI::new(WorkerRIConfig {
            index: 0,
            task_capacity: task_capcity,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            receiver_timeout: receiver_timeout,
            channel_timeout: channel_timeout,
            channel_data: channels.remove(0),
        });
        
        let handles = (1..num_threads).into_iter().zip(channels.into_iter())
            .map(|(index, channel)| {
                let worker = WorkerRI::new(WorkerRIConfig {
                    index: index,
                    task_capacity: task_capcity,
                    share_strategy: share_strategy,
                    wait_strategy: wait_strategy,
                    receiver_timeout: receiver_timeout,
                    channel_timeout: channel_timeout,
                    channel_data: channel,
                });

                thread::spawn(move || {
                    worker.run();
                })
            }).collect::<Vec<_>>();

        PoolRI {
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

pub struct PoolSI {
    local_worker: WorkerSI,
    handles: Vec<thread::JoinHandle<()>>,
}

impl PoolSI {
    pub fn new(num_threads: usize,
               task_capcity: usize,
               share_strategy: ShareStrategy,
               wait_strategy: ReceiverWaitStrategy,
               receiver_timeout: Duration) -> PoolSI {
        assert!(num_threads > 0);

        let mut channels = make_sender_initiated_channels(num_threads);

        let local_worker = WorkerSI::new(WorkerSIConfig {
            index: 0,
            task_capacity: task_capcity,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            receiver_timeout: receiver_timeout,
            channel_data: channels.remove(0),
        });
        
        let handles = (1..num_threads).into_iter().zip(channels.into_iter())
            .map(|(index, channel)| {
                let worker = WorkerSI::new(WorkerSIConfig {
                    index: index,
                    task_capacity: task_capcity,
                    share_strategy: share_strategy,
                    wait_strategy: wait_strategy,
                    receiver_timeout: receiver_timeout,
                    channel_data: channel,
                });

                thread::spawn(move || {
                    worker.run();
                })
            }).collect::<Vec<_>>();

        PoolSI {
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

