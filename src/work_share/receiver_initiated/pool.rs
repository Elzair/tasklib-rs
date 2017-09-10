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
               receiver_timeout: Duration,
               channel_timeout: Duration) -> Pool {
        assert!(num_threads > 0);

        let mut channels = make_channels(num_threads);
        let shared_data = Arc::new(SharedData::new(num_threads));

        // Create `Worker` for main thread.
        let (local_requesters, local_responders) = channels.remove(0);
        let local_worker = Worker::new(WorkerConfig {
            index: 0,
            shared_data: shared_data.clone(),
            task_capacity: task_capacity,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            receiver_timeout: receiver_timeout,
            channel_timeout: channel_timeout,
            requesters: local_requesters,
            responders: local_responders,
        });
        
        let handles = (1..num_threads).into_iter().zip(channels.into_iter())
            .map(|(index, (requesters, responders))| {
                let worker = Worker::new(WorkerConfig {
                    index: index,
                    shared_data: shared_data.clone(),
                    task_capacity: task_capacity,
                    share_strategy: share_strategy,
                    wait_strategy: wait_strategy,
                    receiver_timeout: receiver_timeout,
                    channel_timeout: channel_timeout,
                    requesters: requesters,
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

    // pub fn run(&mut self) {
    //     self.local_worker.run();
    // }

    pub fn run_once(&mut self) {
        self.local_worker.run_once();
    }
}

fn make_channels(num_threads: usize)
                 -> Vec<(Vec<Requester<TaskData>>,
                         Vec<Responder<TaskData>>)>
{

    let nt = num_threads;
    let ntsq = nt * nt;
    
    // Model the channels as several NxN matrices.
    let mut requesters = Vec::<Option<Requester<TaskData>>>::with_capacity(ntsq);
    let mut responders = Vec::<Option<Responder<TaskData>>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match is_diagonal(n, nt) {
            true => {
                requesters.push(None);
                responders.push(None);
            },
            false => {
                let (rqst, resp) = reqchan::channel();
                requesters.push(Some(rqst));
                responders.push(Some(resp));
            },
        }
    }

    // Transpose the `responders` matrix.
    transpose(&mut responders, nt, nt);

    // Decompose the matrices into arrays of rows, filter out all empty
    // entries, and zip together the rows from the two matrices.
    split_and_filter(requesters, nt, nt).into_iter()
        .zip(split_and_filter(responders, nt, nt).into_iter())
        .map(|(rqsts, resps)| { (rqsts, resps) })
        .collect::<Vec<_>>()
}

// This helper function decomposes a matrix of `Option<T>` into an array of rows.
// It then removes any `None`s and unwraps all the `Some`s.
// It returns that filtered array.
#[inline]
fn split_and_filter<T>(mut matrix: Vec<Option<T>>,
                       num_rows: usize,
                       row_size: usize)
                       -> Vec<Vec<T>> {
    assert_eq!(num_rows * row_size, matrix.len());
 
    // Split into 
    let mut tmp = Vec::<Vec<Option<T>>>::with_capacity(num_rows);

    #[allow(unused_variables)]
    for n in 0..num_rows {
        tmp.push(matrix.drain(0..row_size).collect());
    }
   
    // Filter out `None`s, unwrap `Some`s, and return result.
    tmp.into_iter().map(|row| {
        row.into_iter().filter_map(|n| { n }).collect::<Vec<_>>()
    }).collect::<Vec<_>>()
}
                                             
// Return a vector with only Some(T) unwrapped elements.
#[inline]
fn filter_vec<T>(v: Vec<Option<T>>) -> Vec<T> {
    v.into_iter().filter_map(|n| { n }).collect::<Vec<_>>()
}

// Split a `Vec` into `n` different Vecs of length `r`.
#[inline]
fn split_vec<T>(mut v: Vec<T>,
                num_rows: usize,
                row_size: usize)
                -> Vec<Vec<T>> {
    assert_eq!(num_rows * row_size, v.len());

    let mut res = Vec::<Vec<T>>::with_capacity(num_rows);

    #[allow(unused_variables)]
    for n in 0..num_rows {
        res.push(v.drain(0..row_size).collect());
    }

    res
}

#[inline]
fn get_yx(index: usize, row_size: usize) -> (usize, usize) {
    (index / row_size, index % row_size)
}

#[inline]
fn is_diagonal(index: usize, row_size: usize) -> bool {
    let (y, x) = get_yx(index, row_size);
    y == x
}

// Transpose an NxN matrix.
#[inline]
fn transpose<T>(matrix: &mut Vec<T>,
                num_rows: usize,
                row_size: usize) {
    let is_lower_left = |index: usize, row_size: usize| {
        let (y, x) = get_yx(index, row_size);
        y > x
    };

    let get_transpose_index = |index: usize, row_size: usize| {
        let (y, x) = get_yx(index, row_size);
        x * row_size + y 
    };

    assert_eq!(num_rows, row_size);
    
    for n in 0..(num_rows * row_size) {
        if is_lower_left(n, row_size) {
            matrix.swap(n, get_transpose_index(n, row_size));
        }
    }
}


