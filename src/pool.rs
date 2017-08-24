use std::thread;

use super::{ShareStrategy,Initiated};
// use task::Task;
use worker::Worker;
use worker::Config as WorkerConfig;
use channel::make_channels;

pub struct ThreadPool {
    local_worker: Worker,
    handles: Vec<thread::JoinHandle<()>>,
}

impl ThreadPool {
    pub fn new(num_threads:    usize,
               task_capcity:   usize,
               share_strategy: ShareStrategy,
               initiated:      Initiated) -> ThreadPool {
        assert!(num_threads > 0);

        let mut channels = make_channels(num_threads, initiated);

        let local_worker = Worker::new(WorkerConfig {
            index:            0,
            task_capacity:    task_capcity,
            share_strategy:   share_strategy,
            channel_data:     channels.remove(0),
        });
        
        let handles = (1..num_threads).into_iter().zip(channels.into_iter())
            .map(|(index, channel)| {
                let worker = Worker::new(WorkerConfig {
                    index:            index,
                    task_capacity:    task_capcity,
                    share_strategy:   share_strategy,
                    channel_data:     channel,
                });

                thread::spawn(move || {
                    worker.run();
                })
            }).collect::<Vec<_>>();

        ThreadPool {
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

