use std::sync::Arc;
use std::thread;
use std::time::Duration;

use reqchan::{self, Requester, Responder};

use super::super::{ReceiverWaitStrategy, ShareStrategy, TaskData};
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
        let shared_data = Arc::new(SharedData::new(num_threads));

        let (local_requester, local_responders) = channels.remove(0);
        let local_worker = Worker::new(WorkerConfig {
            index: 0,
            shared_data: shared_data.clone(),
            task_capacity: task_capacity,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            receiver_timeout: receiver_timeout,
            requester: local_requester,
            responders: local_responders,
        });
        
        let handles = (1..num_threads).into_iter().zip(channels.into_iter())
            .map(|(index, (requester, responders))| {
                let worker = Worker::new(WorkerConfig {
                    index: index,
                    shared_data: shared_data.clone(),
                    task_capacity: task_capacity,
                    share_strategy: share_strategy,
                    wait_strategy: wait_strategy,
                    receiver_timeout: receiver_timeout,
                    requester: requester,
                    responders: responders,
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

fn make_channels(num_threads: usize)
                 -> Vec<(Requester<TaskData>,
                         Vec<Responder<TaskData>>)>
{
    let nt = num_threads;
    
    let mut requesters = Vec::<Requester<TaskData>>::with_capacity(nt);
    let mut responders1 = Vec::<Responder<TaskData>>::with_capacity(nt);

    // Create initial channels.
    #[allow(unused_variables)]
    for n in 0..nt {
        let (requester, responder) = reqchan::channel::<TaskData>();
        requesters.push(requester);
        responders1.push(responder);
    }

    // Clone `Responder`s N-2 times so there is one `Responder` to give
    // to each of the OTHER workers.
    let mut responders2 = Vec::<Vec<Responder<TaskData>>>::with_capacity(nt);

    #[allow(unused_variables)]
    for n in 0..nt {
        let mut clones = Vec::<Responder<TaskData>>::with_capacity(nt-1);
        let responder = responders1.remove(0);
        
        for nn in 0..(nt-2) {
            clones.push(responder.clone());
        }
        
        clones.push(responder);
        responders2.push(clones);
    }

    // Swap out the `Responder`s so each `Worker` has a `Responder`
    // for every OTHER `Worker`.
    let mut responders3 = Vec::<Vec<Responder<TaskData>>>::with_capacity(nt);

    #[allow(unused_variables)]
    for n in 0..nt {
        let mut worker_responders = Vec::<Responder<TaskData>>::with_capacity(nt-1);

        for nn in 0..nt {
            // Do not get a responder for this worker.
            if nn == n {
                continue;
            }

            worker_responders.push(responders2[nn].pop().unwrap());
        }

        responders3.push(worker_responders);
    }

    // Zip together each entry from `requesters` and `responders3`
    requesters.into_iter().zip(responders3.into_iter())
        .map(|(requester, responders)| {
            (requester, responders)
        }).collect::<Vec<_>>()
}
