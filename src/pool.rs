use std::sync::mpsc;
use std::thread;

use itertools;

use task::Task;
use worker::{Worker,Config,ShareStrategy};

pub struct ThreadPool {
    workers: Option<Vec<Worker>>,
}

impl ThreadPool {
    pub fn new(num_threads:    usize,
               task_capcity:   usize,
               share_strategy: ShareStrategy) -> Result<ThreadPool, ()> {
        assert!(num_threads > 0);

        let mut channels = make_channels(num_threads);
        let mut workers: Vec<Worker> = Vec::new();

        for n in 0..num_threads {
            let new_chans = channels.split_off(num_threads);

            workers.push(Worker::new(Config{ index:          n,
                                             task_capacity:  task_capcity,
                                             share_strategy: share_strategy,
                                             channels:       channels }));

            channels = new_chans;
        }
        
        Ok(ThreadPool{ workers: Some(workers) })
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

fn make_channels(num_threads: usize) -> Vec<(mpsc::Sender<bool>,
                                             mpsc::Receiver<bool>,
                                             mpsc::Sender<usize>,
                                             mpsc::Receiver<usize>,
                                             mpsc::Sender<Task>,
                                             mpsc::Receiver<Task>)> {
    let get_yx = |n: usize, row_size: usize| -> (usize, usize) {
        (n / row_size, n % row_size)
    };

    let is_same_thread = |n: usize, row_size: usize| -> bool {
        let (y, x) = get_yx(n, row_size);
        y == x
    };

    let is_lower_left = |n: usize, row_size: usize| -> bool {
        let (y, x) = get_yx(n, row_size);
        y > x
    };

    let transpose_n = |n: usize, row_size: usize| {
        let (y, x) = get_yx(n, row_size);
        x * row_size + y 
    };

    let ntsq = num_threads * num_threads;
    
    // Model the channels as several NxN matrices.
    let mut rqst_tx = Vec::<Option<mpsc::Sender<bool>>>::with_capacity(ntsq);
    let mut rqst_rx = Vec::<Option<mpsc::Receiver<bool>>>::with_capacity(ntsq);
    let mut resp_tx = Vec::<Option<mpsc::Sender<usize>>>::with_capacity(ntsq);
    let mut resp_rx = Vec::<Option<mpsc::Receiver<usize>>>::with_capacity(ntsq);
    let mut jobs_tx = Vec::<Option<mpsc::Sender<Task>>>::with_capacity(ntsq);
    let mut jobs_rx = Vec::<Option<mpsc::Receiver<Task>>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match is_same_thread(n, num_threads) {
            true => {
                rqst_tx.push(None);
                rqst_rx.push(None);
                resp_tx.push(None);
                resp_rx.push(None);
                jobs_tx.push(None);
                jobs_rx.push(None);
            },
            false => {
                let (tx, rx) = mpsc::channel::<bool>();
                rqst_tx.push(Some(tx));
                rqst_rx.push(Some(rx));
                let (tx, rx) = mpsc::channel::<usize>();
                resp_tx.push(Some(tx));
                resp_rx.push(Some(rx));
                let (tx, rx) = mpsc::channel::<Task>();
                jobs_tx.push(Some(tx));
                jobs_rx.push(Some(rx));
            }
        }
    }

    // Give one part of each channel to its corresponding thread.
    for n in 0..ntsq {
        if is_lower_left(n, num_threads) {
            rqst_rx.swap(n, transpose_n(n, num_threads));
            resp_tx.swap(n, transpose_n(n, num_threads));
            jobs_rx.swap(n, transpose_n(n, num_threads));
        }
    }

    // Remove all Nones and remove each channel pair from its
    // Option container.
    filter!(rqst_tx, rqst_rx,
            resp_tx, resp_rx,
            jobs_tx, jobs_rx);

    itertools::multizip((rqst_tx.into_iter(),
                         rqst_rx.into_iter(),
                         resp_tx.into_iter(),
                         resp_rx.into_iter(),
                         jobs_tx.into_iter(),
                         jobs_rx.into_iter()))
        .collect::<Vec<_>>()
}
