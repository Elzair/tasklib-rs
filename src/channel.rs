use std::sync::mpsc;

use spmc;

use super::Initiated;
use task::Task;

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

pub enum Data {
    Receiver {
        send_requests: Vec<mpsc::Sender<bool>>,
        get_requests: Vec<mpsc::Receiver<bool>>,
        send_responses: Vec<mpsc::Sender<usize>>,
        get_responses: Vec<mpsc::Receiver<usize>>,
        send_tasks: Vec<mpsc::Sender<Task>>,
        get_tasks: Vec<mpsc::Receiver<Task>>,
    },
    Sender {
        send_requests: spmc::Sender<bool>,
        get_requests:  Vec<spmc::Receiver<bool>>,
        send_responses: Vec<mpsc::Sender<usize>>,
        get_responses: Vec<mpsc::Receiver<usize>>,
        send_tasks: Vec<mpsc::Sender<Task>>,
        get_tasks: Vec<mpsc::Receiver<Task>>,
    },
}

pub fn make_channels(num_threads: usize,
                     initiated: Initiated ) -> Vec<Data> {
    match initiated {
        Initiated::RECEIVER => make_channels_ri(num_threads),
        Initiated::SENDER => make_channels_si(num_threads),
    }
}

fn make_channels_ri(num_threads: usize) -> Vec<Data> {
    let ntsq = num_threads * num_threads;
    
    // Model the channels as several NxN matrices.
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

    // Give one part of each channel to its corresponding thread.
    // This is accomplished by transposing the `rqst_rx` 'matrix'.
    for n in 0..ntsq {
        if is_lower_left(n, num_threads) {
            rqst_rx.swap(n, transpose_n(n, num_threads));
        }
    }

    // Remove all Nones and remove each channel pair from its
    // Option container.
    filter!(rqst_tx, rqst_rx);

    let (mut rqst_tx, mut rqst_rx) = (rqst_tx, rqst_rx);
    let (mut resp_tx, mut resp_rx,
         mut jobs_tx, mut jobs_rx) = make_channels_shared(num_threads);
    
    // Create channel data.
    let mut channels = Vec::<Data>::with_capacity(num_threads);

    #[allow(unused_variables)]
    for n in 0..num_threads {
        let rqst_tx_tmp = rqst_tx.split_off(num_threads);
        let send_requests = rqst_tx;
        rqst_tx = rqst_tx_tmp;

        let rqst_rx_tmp = rqst_rx.split_off(num_threads);
        let get_requests = rqst_rx;
        rqst_rx = rqst_rx_tmp;

        let resp_tx_tmp = resp_tx.split_off(num_threads);
        let send_responses = resp_tx;
        resp_tx = resp_tx_tmp;

        let resp_rx_tmp = resp_rx.split_off(num_threads);
        let get_responses = resp_rx;
        resp_rx = resp_rx_tmp;

        let jobs_tx_tmp = jobs_tx.split_off(num_threads);
        let send_tasks = jobs_tx;
        jobs_tx = jobs_tx_tmp;

        let jobs_rx_tmp = jobs_rx.split_off(num_threads);
        let get_tasks = jobs_rx;
        jobs_rx = jobs_rx_tmp;

        channels.push(Data::Receiver {
            send_requests: send_requests,
            get_requests: get_requests,
            send_responses: send_responses,
            get_responses: get_responses,
            send_tasks: send_tasks,
            get_tasks: get_tasks,
        });
    }

    channels
}

fn make_channels_si(num_threads: usize) -> Vec<Data> {
    let mut rqst_tx = Vec::<spmc::Sender<bool>>::with_capacity(num_threads);
    let mut rqst_rx = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads);

    // Create initial channels.
    #[allow(unused_variables)]
    for n in 0..num_threads {
        let (tx, rx) = spmc::channel::<bool>();
        rqst_tx.push(tx);
        rqst_rx.push(rx);
    }

    // Clone Receivers N-2 times so there is one Receiver to give
    // to each of the OTHER workers.
    let mut rqst_rx_tmp = Vec::<Vec<spmc::Receiver<bool>>>::with_capacity(num_threads);

    #[allow(unused_variables)]
    for n in 0..num_threads {
        let mut clones = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads - 1);
        let rx = rqst_rx.remove(0);
        
        for nn in 0..(num_threads-2) {
            clones.push(rx.clone());
        }
        
        clones.push(rx);
        rqst_rx_tmp.push(clones);
    }

    // Swap out the receivers so each worker has a receiver for every OTHER worker.
    let mut rqst_rx = Vec::<Vec<spmc::Receiver<bool>>>::with_capacity(num_threads);

    #[allow(unused_variables)]
    for n in 0..num_threads {
        let mut recvs = Vec::<spmc::Receiver<bool>>::with_capacity(num_threads - 1);

        for nn in 0..num_threads {
            // Do not get a receiver for this worker.
            if nn == n {
                continue;
            }

            recvs.push(rqst_rx_tmp[nn].pop().unwrap());
        }

        rqst_rx.push(recvs);
    }

    let (mut resp_tx, mut resp_rx,
         mut jobs_tx, mut jobs_rx) = make_channels_shared(num_threads);
    
    // Create channel data.
    let mut channels = Vec::<Data>::with_capacity(num_threads);

    #[allow(unused_variables)]
    for n in 0..num_threads {
        let send_requests = rqst_tx.remove(0);

        let get_requests = rqst_rx.remove(0);

        let resp_tx_tmp = resp_tx.split_off(num_threads);
        let send_responses = resp_tx;
        resp_tx = resp_tx_tmp;

        let resp_rx_tmp = resp_rx.split_off(num_threads);
        let get_responses = resp_rx;
        resp_rx = resp_rx_tmp;

        let jobs_tx_tmp = jobs_tx.split_off(num_threads);
        let send_tasks = jobs_tx;
        jobs_tx = jobs_tx_tmp;

        let jobs_rx_tmp = jobs_rx.split_off(num_threads);
        let get_tasks = jobs_rx;
        jobs_rx = jobs_rx_tmp;

        channels.push(Data::Sender {
            send_requests: send_requests,
            get_requests: get_requests,
            send_responses: send_responses,
            get_responses: get_responses,
            send_tasks: send_tasks,
            get_tasks: get_tasks,
        });
    }

    channels
}

fn make_channels_shared(num_threads: usize)
                        -> (Vec<mpsc::Sender<usize>>,
                            Vec<mpsc::Receiver<usize>>,
                            Vec<mpsc::Sender<Task>>,
                            Vec<mpsc::Receiver<Task>>)
{
    let ntsq = num_threads * num_threads;
    
    // Model the channels as several NxN matrices.
    let mut resp_tx = Vec::<Option<mpsc::Sender<usize>>>::with_capacity(ntsq);
    let mut resp_rx = Vec::<Option<mpsc::Receiver<usize>>>::with_capacity(ntsq);
    let mut jobs_tx = Vec::<Option<mpsc::Sender<Task>>>::with_capacity(ntsq);
    let mut jobs_rx = Vec::<Option<mpsc::Receiver<Task>>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match is_same_thread(n, num_threads) {
            true => {
                resp_tx.push(None);
                resp_rx.push(None);
                jobs_tx.push(None);
                jobs_rx.push(None);
            },
            false => {
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
    // This is accomplished by transposing the `resp_tx` & `jobs_tx` 'matrices'.
    for n in 0..ntsq {
        if is_lower_left(n, num_threads) {
            resp_tx.swap(n, transpose_n(n, num_threads));
            jobs_rx.swap(n, transpose_n(n, num_threads));
        }
    }

    // Remove all Nones and remove each channel pair from its
    // Option container.
    filter!(resp_tx, resp_rx,
            jobs_tx, jobs_rx);

    (resp_tx, resp_rx, jobs_tx, jobs_rx)
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

