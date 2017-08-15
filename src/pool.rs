use std::sync::mpsc;

use itertools;

use task::Task;
use worker::{Worker,Config};

pub struct ThreadPool {
    workers: Vec<Worker>,
}

impl ThreadPool {
    pub fn new(num_threads: usize) -> Result<ThreadPool, ()> {

        let mut channels = make_channels(num_threads);
        let mut workers: Vec<Worker> = Vec::new();

        for n in 0..num_threads {
            let new_chans = channels.split_off(num_threads);

            workers.push(Worker::new(Config{ index: n,
                                             task_capacity: 64,
                                             channels: channels }));

            channels = new_chans;
        }
        
        Ok(ThreadPool{ workers })
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
                                             mpsc::Sender<bool>,
                                             mpsc::Receiver<bool>,
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

    // let get_n = |y: usize, x: usize, row_size: usize| {
    //     y * row_size + x
    // };

    let ntsq = num_threads * num_threads;
    
    // Model the channels as an nxn matrix.
    let mut rqst_tx = Vec::<Option<mpsc::Sender<bool>>>::with_capacity(ntsq);
    let mut rqst_rx = Vec::<Option<mpsc::Receiver<bool>>>::with_capacity(ntsq);
    let mut resp_tx = Vec::<Option<mpsc::Sender<bool>>>::with_capacity(ntsq);
    let mut resp_rx = Vec::<Option<mpsc::Receiver<bool>>>::with_capacity(ntsq);
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
                let (tx, rx) = mpsc::channel::<bool>();
                resp_tx.push(Some(tx));
                resp_rx.push(Some(rx));
                let (tx, rx) = mpsc::channel::<Task>();
                jobs_tx.push(Some(tx));
                jobs_rx.push(Some(rx));
            }
        }
    }

    // let coords =
    //     (0..ntsq).map(|n| {
    //         (n / num_threads, n % num_threads)
    //     }).filter(|&(y, x)| {
    //         y > x
    //     }).collect::<Vec<_>>();

    
    // for &(y, x) in coords.iter() {
    //     rqst_rx.swap(get_n(y, x, num_threads),
    //                  get_n(x, y, num_threads));
    //     resp_tx.swap(get_n(y, x, num_threads),
    //                  get_n(x, y, num_threads));
    //     jobs_rx.swap(get_n(y, x, num_threads),
    //                  get_n(x, y, num_threads));
    // }

    for n in 0..ntsq {
        if is_lower_left(n, num_threads) {
            rqst_rx.swap(n, transpose_n(n, num_threads));
            resp_tx.swap(n, transpose_n(n, num_threads));
            jobs_rx.swap(n, transpose_n(n, num_threads));
        }
    }

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