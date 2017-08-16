use std::sync::mpsc;
use std::thread;

use spmc;
use itertools;

use super::{ShareStrategy,Initiated};
use task::Task;
use worker::{Worker, InitiatedConfig};
use worker::Config as WorkerConfig;

pub struct ThreadPool {
    workers: Option<Vec<Worker>>,
}

impl ThreadPool {
    pub fn new(num_threads:    usize,
               task_capcity:   usize,
               share_strategy: ShareStrategy,
               initiated:      Initiated) -> ThreadPool {
        assert!(num_threads > 0);

        let mut channels = make_channels(num_threads);
        let init_chans = make_initiated_channels(num_threads, initiated);
        let mut workers: Vec<Worker> = Vec::new();

        match init_chans {
            TempConf::Receiver(mut data) => {
                for n in 0..num_threads {
                    // Take the first N elements from `channels`.
                    let new_chans = channels.split_off(num_threads);
                    let chtmp = channels;
                    channels = new_chans;

                    // Ditto for `data`. 
                    let new_data = data.split_off(num_threads);
                    let dtmp = data;
                    let init_cfg = InitiatedConfig::Receiver( dtmp );
                    data = new_data;

                    workers.push(Worker::new(WorkerConfig{
                        index:            n,
                        task_capacity:    task_capcity,
                        share_strategy:   share_strategy,
                        initiated_config: init_cfg,
                        channels:         chtmp,
                    }));
                }
            },
            TempConf::Sender(mut sender, mut receivers) => {
                for n in 0..num_threads {
                    // See above.
                    let new_chans = channels.split_off(num_threads);
                    let chtmp = channels;
                    channels = new_chans;

                    // Get a receiver for each of the OTHER threads.
                    let mut new_recvs = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads - 1);

                    for nn in 0..num_threads {
                        // Do not get a receiver for this worker.
                        if nn == n {
                            continue;
                        }

                        new_recvs.push(receivers[nn].pop().unwrap());
                    }
                    
                    let init_cfg = InitiatedConfig::Sender {
                        send_requests: sender.pop().unwrap(),
                        get_requests: new_recvs,
                    };

                    workers.push(Worker::new(WorkerConfig{
                        index:            n,
                        task_capacity:    task_capcity,
                        share_strategy:   share_strategy,
                        initiated_config: init_cfg,
                        channels:         chtmp,
                    }));
                }

            },
        }

        // for n in 0..num_threads {
        //     let new_chans = channels.split_off(num_threads);
        //     let chtmp = channels;
        //     channels = new_chans;

        //     let init_cfg = match init_chans {
        //         TempConf::Receiver( mut data ) => {
        //             // Take the first N elements from data and leave the rest for another loop.
        //             let new_data = data.split_off(num_threads);
        //             let dtmp = data;
        //             init_chans = TempConf::Receiver(new_data);
        //             InitiatedConfig::Receiver( dtmp )
        //         },
        //         TempConf::Sender( mut sender, mut receivers ) => {
        //             let mut new_recvs = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads - 1);

        //             // Get a receiver for each of the other threads.
        //             for nn in 0..num_threads {
        //                 // Do not get a receiver for this worker.
        //                 if nn == n {
        //                     continue;
        //                 }

        //                 new_recvs.push(receivers[nn].pop().unwrap());
        //             }
                    
        //             InitiatedConfig::Sender {
        //                 send_requests: sender.pop().unwrap(),
        //                 get_requests: new_recvs,
        //             }
        //         },
        //     };

        //     workers.push(Worker::new(WorkerConfig{
        //         index:            n,
        //         task_capacity:    task_capcity,
        //         share_strategy:   share_strategy,
        //         initiated_config: init_cfg,
        //         //channels:         channels
        //         channels:         chtmp,
        //     }));

        //     //channels = new_chans;
        // }
        
        ThreadPool{ workers: Some(workers) }
    }

    pub fn run(&mut self) {
        assert!(self.workers.is_some());
        let mut workers = self.workers.take().unwrap();
        let new_workers = workers.split_off(1);
        
        let handles = new_workers.into_iter()
            .map(|mut worker| {
                thread::spawn(move || {
                    worker.run();
                })
            }).collect::<Vec<_>>();

        workers.pop().unwrap().run();

        for handle in handles.into_iter() {
            handle.join().expect("Could not join child thread");
        }
    }
}

macro_rules! filter {
    ($vec_name:ident) => (
        let $vec_name = $vec_name.into_iter()
            .filter_map(|n| { n })
            .collect::<Vec<_>>();
    );
    ($vec_name:ident, $($rest:ident),+) => (
        filter!($vec_name); 
        filter!($($rest),+);
    )
}

fn make_channels(num_threads: usize) -> Vec<(// mpsc::Sender<bool>,
    // mpsc::Receiver<bool>,
    mpsc::Sender<usize>,
    mpsc::Receiver<usize>,
    mpsc::Sender<Task>,
    mpsc::Receiver<Task>)> {
    let ntsq = num_threads * num_threads;
    
    // Model the channels as several NxN matrices.
    // let mut rqst_tx = Vec::<Option<mpsc::Sender<bool>>>::with_capacity(ntsq);
    // let mut rqst_rx = Vec::<Option<mpsc::Receiver<bool>>>::with_capacity(ntsq);
    let mut resp_tx = Vec::<Option<mpsc::Sender<usize>>>::with_capacity(ntsq);
    let mut resp_rx = Vec::<Option<mpsc::Receiver<usize>>>::with_capacity(ntsq);
    let mut jobs_tx = Vec::<Option<mpsc::Sender<Task>>>::with_capacity(ntsq);
    let mut jobs_rx = Vec::<Option<mpsc::Receiver<Task>>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match is_same_thread(n, num_threads) {
            true => {
                // rqst_tx.push(None);
                // rqst_rx.push(None);
                resp_tx.push(None);
                resp_rx.push(None);
                jobs_tx.push(None);
                jobs_rx.push(None);
            },
            false => {
                // let (tx, rx) = mpsc::channel::<bool>();
                // rqst_tx.push(Some(tx));
                // rqst_rx.push(Some(rx));
                let (tx, rx) = mpsc::channel::<usize>();
                resp_tx.push(Some(tx));
                resp_rx.push(Some(rx));
                let (tx, rx) = mpsc::channel::<Task>();
                jobs_tx.push(Some(tx));
                jobs_rx.push(Some(rx));
            },
        }
    }

    // Give one part of each channel to its corresponding thread.
    for n in 0..ntsq {
        if is_lower_left(n, num_threads) {
            // rqst_rx.swap(n, transpose_n(n, num_threads));
            resp_tx.swap(n, transpose_n(n, num_threads));
            jobs_rx.swap(n, transpose_n(n, num_threads));
        }
    }

    // Remove all Nones and remove each channel pair from its
    // Option container.
    // filter!(rqst_tx, rqst_rx,
    //         resp_tx, resp_rx,
    //         jobs_tx, jobs_rx);
    filter!(resp_tx, resp_rx, jobs_tx, jobs_rx);

    itertools::multizip((// rqst_tx.into_iter(),
        // rqst_rx.into_iter(),
        resp_tx.into_iter(),
        resp_rx.into_iter(),
        jobs_tx.into_iter(),
        jobs_rx.into_iter()))
        .collect::<Vec<_>>()
}

enum TempConf {
    Receiver (
        Vec<(mpsc::Sender<bool>, mpsc::Receiver<bool>)>,
    ),
    Sender (
        Vec<spmc::Sender<bool>>,
        Vec<Vec<spmc::Receiver<bool>>>,
    ),
}

fn make_initiated_channels(num_threads: usize,
                           initiated:   Initiated) -> TempConf {

    let ntsq = num_threads * num_threads;

    match initiated {
        Initiated::RECEIVER => {
            // This code is similar to `make_channels()`.
            let mut rqst_tx = Vec::<Option<mpsc::Sender<bool>>>::with_capacity(ntsq);
            let mut rqst_rx = Vec::<Option<mpsc::Receiver<bool>>>::with_capacity(ntsq);

            for n in 0..ntsq {
                match is_same_thread(n, num_threads) {
                    true => {
                        rqst_tx.push(None);
                        rqst_rx.push(None);
                    },
                    false => {
                        let (tx, rx) = mpsc::channel::<bool>();
                        rqst_tx.push(Some(tx));
                        rqst_rx.push(Some(rx));
                    },
                }
            }

            for n in 0..ntsq {
                if is_lower_left(n, num_threads) {
                    rqst_rx.swap(n, transpose_n(n, num_threads));
                }
            }

            filter!(rqst_tx, rqst_rx);

            TempConf::Receiver( itertools::multizip((rqst_tx.into_iter(),
                                                     rqst_rx.into_iter()))
                                .collect::<Vec<_>>() )
        },
        Initiated::SENDER => {
            let mut rqst_tx = Vec::<spmc::Sender<bool>>::with_capacity(num_threads);
            let mut rqst_rx = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads);

            // Create initial channels
            #[allow(unused_variables)]
            for n in 0..num_threads {
                let (tx, rx) = spmc::channel::<bool>();
                rqst_tx.push(tx);
                rqst_rx.push(rx);
            }

            // Clone Receivers N-2 times so there is one Receiver to give
            // to each of the OTHER workers.
            let mut new_rx = Vec::<Vec<spmc::Receiver<bool>>>::with_capacity(num_threads);

            #[allow(unused_variables)]
            for n in 0..num_threads {
                let mut clones = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads - 1);
                let rx = rqst_rx.remove(0);
                
                #[allow(unused_variables)]
                for nn in 0..(num_threads-2) {
                    clones.push(rx.clone());
                }
                
                clones.push(rx);
                new_rx.push(clones);
            }

            TempConf::Sender( rqst_tx, new_rx )
        },
    }
}

fn get_yx(n: usize, row_size: usize) -> (usize, usize) {
    (n / row_size, n % row_size)
}

fn is_same_thread(n: usize, row_size: usize) -> bool {
    let (y, x) = get_yx(n, row_size);
    y == x
}

fn is_lower_left(n: usize, row_size: usize) -> bool {
    let (y, x) = get_yx(n, row_size);
    y > x
}

fn transpose_n(n: usize, row_size: usize) -> usize {
    let (y, x) = get_yx(n, row_size);
    x * row_size + y 
}

