use std::sync::mpsc;

use itertools;
use spmc;

use super::Initiated;
use task::Task;

pub enum Data
{
    Receiver
    {
        send_requests: Vec<mpsc::Sender<bool>>,
        get_requests: Vec<mpsc::Receiver<bool>>,
        send_responses: Vec<mpsc::Sender<usize>>,
        get_responses: Vec<mpsc::Receiver<usize>>,
        send_tasks: Vec<mpsc::Sender<Task>>,
        get_tasks: Vec<mpsc::Receiver<Task>>,
    },
    Sender
    {
        send_requests: spmc::Sender<bool>,
        get_requests: Vec<spmc::Receiver<bool>>,
        send_responses: Vec<mpsc::Sender<usize>>,
        get_responses: Vec<mpsc::Receiver<usize>>,
        send_tasks: Vec<mpsc::Sender<Task>>,
        get_tasks: Vec<mpsc::Receiver<Task>>,
    },
}

pub fn make_channels(num_threads: usize,
                     initiated: Initiated ) -> Vec<Data>
{
    let (resp_tx, resp_rx,
         jobs_tx, jobs_rx) = make_channels_shared(num_threads);
    
    match initiated {
        Initiated::RECEIVER => {
            let (rqst_tx, rqst_rx) = make_channels_ri(num_threads);

            itertools::multizip((rqst_tx, rqst_rx,
                                 resp_tx, resp_rx,
                                 jobs_tx, jobs_rx))
                .map(|(send_requests, get_requests,
                       send_responses, get_responses,
                       send_tasks, get_tasks)| {
                    Data::Receiver {
                        send_requests: send_requests,
                        get_requests: get_requests,
                        send_responses: send_responses,
                        get_responses: get_responses,
                        send_tasks: send_tasks,
                        get_tasks: get_tasks,
                    }
                }).collect::<Vec<Data>>()
        },
        Initiated::SENDER => {
            let (rqst_tx, rqst_rx) = make_channels_si(num_threads);

            itertools::multizip((rqst_tx, rqst_rx,
                                 resp_tx, resp_rx,
                                 jobs_tx, jobs_rx))
                .map(|(send_requests, get_requests,
                       send_responses, get_responses,
                       send_tasks, get_tasks)| {
                    Data::Sender {
                        send_requests: send_requests,
                        get_requests: get_requests,
                        send_responses: send_responses,
                        get_responses: get_responses,
                        send_tasks: send_tasks,
                        get_tasks: get_tasks,
                    }
                }).collect::<Vec<Data>>()
        },
    }
}

fn make_channels_ri(num_threads: usize)
                    -> (Vec<Vec<mpsc::Sender<bool>>>,
                        Vec<Vec<mpsc::Receiver<bool>>>)
{
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

    (
        split_vec(filter_vec(rqst_tx), num_threads, num_threads-1),
        split_vec(filter_vec(rqst_rx), num_threads, num_threads-1),
    )
}

fn make_channels_si(num_threads: usize)
                    -> (Vec<spmc::Sender<bool>>,
                        Vec<Vec<spmc::Receiver<bool>>>)
{
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

    (rqst_tx, rqst_rx)
}

fn make_channels_shared(num_threads: usize)
                        -> (Vec<Vec<mpsc::Sender<usize>>>,
                            Vec<Vec<mpsc::Receiver<usize>>>,
                            Vec<Vec<mpsc::Sender<Task>>>,
                            Vec<Vec<mpsc::Receiver<Task>>>)
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

    // Remove all Nones, remove each channel pair from its
    // Option container, and split the Vec into num_threads pieces.
    (
        split_vec(filter_vec(resp_tx), num_threads, num_threads-1),
        split_vec(filter_vec(resp_rx), num_threads, num_threads-1),
        split_vec(filter_vec(jobs_tx), num_threads, num_threads-1),
        split_vec(filter_vec(jobs_rx), num_threads, num_threads-1),
    )
}

fn filter_vec<T>(v: Vec<Option<T>>) -> Vec<T> {
    v.into_iter().filter_map(|n| { n }).collect::<Vec<_>>()
}

fn split_vec<T>(mut v: Vec<T>, n: usize, r: usize) -> Vec<Vec<T>> {
    assert!(n*r == v.len());

    let mut res = Vec::<Vec<T>>::with_capacity(n);

    #[allow(unused_variables)]
    for nn in 0..n {
        res.push(v.drain(0..r).collect());
    }

    res
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

#[cfg(test)]
mod tests {
    use super::super::Initiated;
    use super::{Data, make_channels, make_channels_ri, make_channels_si, make_channels_shared, split_vec};

    static NT: usize = 4;

    #[test]
    fn test_split_vec() {
        let vec = vec![0, 1, 2,
                           3, 4, 5,
                           6, 7, 8,
                           9, 10, 11];
        let (n, r) = (4, 3);
        let vec2 = split_vec(vec, 4, 3);
        assert!(vec2.len() == n);
        for vec3 in vec2 {
            assert!(vec3.len() == r);
        }
    }

    #[test]
    fn test_make_channels_shared() {
        let (resp_tx, resp_rx,
             jobs_tx, jobs_rx) = make_channels_shared(NT);
        assert!(resp_tx.len() == NT);
        assert!(resp_rx.len() == NT);
        assert!(jobs_tx.len() == NT);
        assert!(jobs_rx.len() == NT);

        for v in resp_tx.iter() {
            assert!(v.len() == NT-1);
        }

        for v in resp_rx.iter() {
            assert!(v.len() == NT-1);
        }
        
        for v in jobs_tx.iter() {
            assert!(v.len() == NT-1);
        }

        for v in jobs_rx.iter() {
            assert!(v.len() == NT-1);
        }
    }

    #[test]
    fn test_make_channels_ri() {
        let (rqst_tx, rqst_rx) = make_channels_ri(NT);

        assert!(rqst_tx.len() == NT);
        assert!(rqst_rx.len() == NT);

        for v in rqst_tx.iter() {
            assert!(v.len() == NT-1);
        }

        for v in rqst_rx.iter() {
            assert!(v.len() == NT-1);
        }
    }

    #[test]
    fn test_make_channels_si() {
        let (rqst_tx, rqst_rx) = make_channels_si(NT);

        assert!(rqst_tx.len() == NT);
        assert!(rqst_rx.len() == NT);

        for v in rqst_rx.iter() {
            assert!(v.len() == NT-1);
        }
    }

    #[test]
    fn test_make_ri() {
        let data = make_channels(NT, Initiated::RECEIVER);

        assert!(data.len() == NT);

        for datum in data.into_iter() {
            match datum {
                Data::Receiver {
                    send_requests, get_requests,
                    send_responses, get_responses,
                    send_tasks, get_tasks,
                } => {
                    assert!(send_requests.len() == NT-1);
                    assert!(get_requests.len() == NT-1);
                    assert!(send_responses.len() == NT-1);
                    assert!(get_responses.len() == NT-1);
                    assert!(send_tasks.len() == NT-1);
                    assert!(get_tasks.len() == NT-1);
                },
                Data::Sender { .. } => {
                    assert!(false);
                }
            }
        }
    }

    #[test]
    fn test_make_si() {
        let data = make_channels(NT, Initiated::SENDER);

        assert!(data.len() == NT);

        for datum in data.into_iter() {
            match datum {
                Data::Sender {
                    get_requests,
                    send_responses, get_responses,
                    send_tasks, get_tasks, ..
                } => {
                    assert!(get_requests.len() == NT-1);
                    assert!(send_responses.len() == NT-1);
                    assert!(get_responses.len() == NT-1);
                    assert!(send_tasks.len() == NT-1);
                    assert!(get_tasks.len() == NT-1);
                },
                Data::Receiver { .. } => { assert!(false); },
            }
        }
    }

    #[test]
    fn test_chans_ri() {
        let mut data = make_channels(2, Initiated::RECEIVER);
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        match chan1 {
            Data::Receiver {
                ref send_requests, ..
            } => {
                send_requests[0].send(true).unwrap();
            },
            Data::Sender { .. } => { assert!(false); },
        };

        match chan2 {
            Data::Receiver {
                ref get_requests, ref send_responses, ref send_tasks, ..
            } => {
                let req = get_requests[0].recv().unwrap();
                assert!(req == true);
                send_responses[0].send(1).unwrap();
                send_tasks[0].send(Box::new(|| {
                    println!("Hello!");
                }));
            },
            Data::Sender { .. } => { assert!(false); },
        };

        match chan1 {
            Data::Receiver {
                ref get_responses, ref get_tasks, ..
            } => {
                let res1 = get_responses[0].recv().unwrap();
                assert!(res1 == 1);
                let res2 = get_tasks[0].recv().unwrap();
                res2.call_box();
            },
            Data::Sender { .. } => { assert!(false); },
        };

        match chan2 {
            Data::Receiver {
                ref send_requests, ..
            } => {
                send_requests[0].send(true).unwrap();
            },
            Data::Sender { .. } => { assert!(false); },
        };

        match chan1 {
            Data::Receiver {
                ref get_requests, ref send_responses, ref send_tasks, ..
            } => {
                let req = get_requests[0].recv().unwrap();
                assert!(req == true);
                send_responses[0].send(1).unwrap();
                send_tasks[0].send(Box::new(|| {
                    println!("World!");
                }));
            },
            Data::Sender { .. } => { assert!(false); },
        };

        match chan2 {
            Data::Receiver {
                ref get_responses, ref get_tasks, ..
            } => {
                let res1 = get_responses[0].recv().unwrap();
                assert!(res1 == 1);
                let res2 = get_tasks[0].recv().unwrap();
                res2.call_box();
            },
            Data::Sender { .. } => { assert!(false); },
        };
    }

    #[test]
    fn test_chans_si() {
        let mut data = make_channels(2, Initiated::SENDER);
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        match chan1 {
            Data::Sender {
                ref send_requests, ..
            } => {
                send_requests.send(true).unwrap();
            },
            Data::Receiver { .. } => { assert!(false); },
        };

        match chan2 {
            Data::Sender {
                ref get_requests, ref send_responses, ref send_tasks, ..
            } => {
                let req = get_requests[0].recv().unwrap();
                assert!(req == true);
                send_responses[0].send(1).unwrap();
                send_tasks[0].send(Box::new(|| {
                    println!("Hello!");
                }));
            },
            Data::Receiver { .. } => { assert!(false); },
        };

        match chan1 {
            Data::Sender {
                ref get_responses, ref get_tasks, ..
            } => {
                let res1 = get_responses[0].recv().unwrap();
                assert!(res1 == 1);
                let res2 = get_tasks[0].recv().unwrap();
                res2.call_box();
            },
            Data::Receiver { .. } => { assert!(false); },
        };

        match chan2 {
            Data::Sender {
                ref send_requests, ..
            } => {
                send_requests.send(true).unwrap();
            },
            Data::Receiver { .. } => { assert!(false); },
        };

        match chan1 {
            Data::Sender {
                ref get_requests, ref send_responses, ref send_tasks, ..
            } => {
                let req = get_requests[0].recv().unwrap();
                assert!(req == true);
                send_responses[0].send(1).unwrap();
                send_tasks[0].send(Box::new(|| {
                    println!("World!");
                }));
            },
            Data::Receiver { .. } => { assert!(false); },
        };

        match chan2 {
            Data::Sender {
                ref get_responses, ref get_tasks, ..
            } => {
                let res1 = get_responses[0].recv().unwrap();
                assert!(res1 == 1);
                let res2 = get_tasks[0].recv().unwrap();
                res2.call_box();
            },
            Data::Receiver { .. } => { assert!(false); },
        };
    }
}
