use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
        let shared_data = Arc::new(SharedData::new(num_threads, capacity));

        let local_worker = Worker::new(WorkerConfig {
            index: 0,
            shared_data: shared_data.clone(),
            timeout: timeout,
        });

        
        let handles = (1..num_threads).into_iter()
            .map(|index| {
                let worker = Worker::new(WorkerConfig {
                    index: index,
                    shared_data: shared_data.clone(),
                    timeout: timeout,
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

    // pub fn run(&mut self) {
    //     self.local_worker.run();
    // }

    pub fn run_once(&mut self) {
        self.local_worker.run_once();
    }
}
