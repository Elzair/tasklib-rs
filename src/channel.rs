use std::time::Duration;

use itertools;

use super::{Initiated, ReceiverWaitStrategy};
use super::boolchan as bc;
use super::taskchan as tc;

pub enum Channel {
    Receiver {
        request_send: bc::Sender,
        request_get: bc::Receiver,
        task_send: tc::Sender,
        task_get: tc::Receiver,
    },
    Sender {
        request_get: bc::Receiver,
        task_send: tc::Sender,
        task_get: tc::Receiver,
    },
}

pub enum Data {
    Receiver {
        channels: Vec<Channel>,
    },
    Sender {
        request_send: bc::Sender,
        channels: Vec<Channel>,
    },
}

pub fn make_channels(num_threads: usize,
                     initiated: Initiated) -> Vec<Data>
{
    let (tasks_tx, tasks_rx) = make_channels_shared(num_threads);
    
    match initiated {
        Initiated::Receiver => {
            let (rqsts_tx, rqsts_rx) = make_channels_ri(num_threads);

            itertools::multizip((rqsts_tx, rqsts_rx,
                                 tasks_tx, tasks_rx))
                .map(|(requests_send, requests_get,
                       tasks_send, tasks_get)| {
                    Data::Receiver {
                        channels: itertools::multizip((requests_send,
                                                       requests_get,
                                                       tasks_send,
                                                       tasks_get))
                            .map(|(request_send, request_get,
                                   task_send, task_get)| {
                                Channel::Receiver {
                                    request_send: request_send,
                                    request_get: request_get,
                                    task_send: task_send,
                                    task_get: task_get,
                                }
                            }).collect::<Vec<Channel>>()
                    }
                }).collect::<Vec<Data>>()
        },
        Initiated::Sender => {
            let (rqsts_tx, rqsts_rx) = make_channels_si(num_threads);

            itertools::multizip((rqsts_tx, rqsts_rx,
                                 tasks_tx, tasks_rx))
                .map(|(request_send, requests_get,
                       tasks_send, tasks_get)| {
                    Data::Sender {
                        request_send: request_send,
                        channels: itertools::multizip((requests_get,
                                                       tasks_send,
                                                       tasks_get))
                            .map(|(request_get, task_send, task_get)| {
                                Channel::Sender {
                                    request_get: request_get,
                                    task_send: task_send,
                                    task_get: task_get,
                                }
                            }).collect::<Vec<Channel>>(),
                    }
                }).collect::<Vec<Data>>()
        },
    }
}

fn make_channels_ri(num_threads: usize)
                    -> (Vec<Vec<bc::Sender>>,
                        Vec<Vec<bc::Receiver>>)
{
    let ntsq = num_threads * num_threads;
    
    // Model the channels as several NxN matrices.
    let mut rqst_tx = Vec::<Option<bc::Sender>>::with_capacity(ntsq);
    let mut rqst_rx = Vec::<Option<bc::Receiver>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match is_same_thread(n, num_threads) {
            true => {
                rqst_tx.push(None);
                rqst_rx.push(None);
            },
            false => {
                let (tx, rx) = bc::make_boolchan();
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

    // Remove all `Nones`, remove each channel pair from its
    // `Option` container, and split the Vec into `num_threads` pieces.
    (
        split_vec(filter_vec(rqst_tx), num_threads, num_threads-1),
        split_vec(filter_vec(rqst_rx), num_threads, num_threads-1),
    )
}

fn make_channels_si(num_threads: usize)
                    -> (Vec<bc::Sender>,
                        Vec<Vec<bc::Receiver>>)
{
    let mut rqst_tx = Vec::<bc::Sender>::with_capacity(num_threads);
    let mut rqst_rx = Vec::<bc::Receiver>::with_capacity(num_threads);

    // Create initial channels.
    #[allow(unused_variables)]
    for n in 0..num_threads {
        let (tx, rx) = bc::make_boolchan();
        rqst_tx.push(tx);
        rqst_rx.push(rx);
    }

    // Clone Receivers N-2 times so there is one Receiver to give
    // to each of the OTHER workers.
    let mut rqst_rx_tmp = Vec::<Vec<bc::Receiver>>::with_capacity(num_threads);

    #[allow(unused_variables)]
    for n in 0..num_threads {
        let mut clones = Vec::<bc::Receiver>::with_capacity(num_threads - 1);
        let rx = rqst_rx.remove(0);
        
        for nn in 0..(num_threads-2) {
            clones.push(rx.clone());
        }
        
        clones.push(rx);
        rqst_rx_tmp.push(clones);
    }

    // Swap out the receivers so each worker has a receiver for every OTHER worker.
    let mut rqst_rx = Vec::<Vec<bc::Receiver>>::with_capacity(num_threads);

    #[allow(unused_variables)]
    for n in 0..num_threads {
        let mut recvs = Vec::<bc::Receiver>::with_capacity(num_threads - 1);

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
                        -> (Vec<Vec<tc::Sender>>,
                            Vec<Vec<tc::Receiver>>)
{
    let ntsq = num_threads * num_threads;
    
    // Model the channels as several NxN matrices.
    let mut tasks_tx = Vec::<Option<tc::Sender>>::with_capacity(ntsq);
    let mut tasks_rx = Vec::<Option<tc::Receiver>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match is_same_thread(n, num_threads) {
            true => {
                tasks_tx.push(None);
                tasks_rx.push(None);
            },
            false => {
                let (tx, rx) = tc::make_taskchan();
                tasks_tx.push(Some(tx));
                tasks_rx.push(Some(rx));
            },
        }
    }

    // Give one part of each channel to its corresponding thread.
    // This is accomplished by transposing the `tasks_rx` 'matrices'.
    for n in 0..ntsq {
        if is_lower_left(n, num_threads) {
            tasks_rx.swap(n, transpose_n(n, num_threads));
        }
    }

    // Remove all `Nones`, remove each channel pair from its
    // `Option` container, and split the Vec into `num_threads` pieces.
    (
        split_vec(filter_vec(tasks_tx), num_threads, num_threads-1),
        split_vec(filter_vec(tasks_rx), num_threads, num_threads-1),
    )
}

// Helper functions shared between `make_channels_ri()` and `make_channels_shared`

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
    // use super::super::task::{Task, FnBox};
    use super::super::task::Data as TaskData;

    use super::{Data, Channel, make_channels, make_channels_ri, make_channels_si, make_channels_shared, split_vec};

    static NT: usize = 4;

    #[test]
    fn test_split_vec() {
        let vec = vec![0, 1, 2,
                           3, 4, 5,
                           6, 7, 8,
                           9, 10, 11];
        let (n, r) = (4, 3);
        let vec2 = split_vec(vec, 4, 3);
        assert_eq!(vec2.len(), n);
        for vec3 in vec2 {
            assert_eq!(vec3.len(), r);
        }
    }

    #[test]
    fn test_make_channels_shared() {
        let (tasks_tx, tasks_rx) = make_channels_shared(NT);
        assert_eq!(tasks_tx.len(), NT);
        assert_eq!(tasks_rx.len(), NT);
        
        for v in tasks_tx.iter() {
            assert_eq!(v.len(), NT-1);
        }

        for v in tasks_rx.iter() {
            assert_eq!(v.len(), NT-1);
        }
    }

    #[test]
    fn test_make_channels_ri() {
        let (rqst_tx, rqst_rx) = make_channels_ri(NT);

        assert_eq!(rqst_tx.len(), NT);
        assert_eq!(rqst_rx.len(), NT);

        for v in rqst_tx.iter() {
            assert_eq!(v.len(), NT-1);
        }

        for v in rqst_rx.iter() {
            assert_eq!(v.len(), NT-1);
        }
    }

    #[test]
    fn test_make_channels_si() {
        let (rqst_tx, rqst_rx) = make_channels_si(NT);

        assert_eq!(rqst_tx.len(), NT);
        assert_eq!(rqst_rx.len(), NT);

        for v in rqst_rx.iter() {
            assert_eq!(v.len(), NT-1);
        }
    }

    #[test]
    fn test_make_ri() {
        let data = make_channels(NT, Initiated::Receiver);

        assert_eq!(data.len(), NT);

        for datum in data.into_iter() {
            match datum {
                Data::Receiver {
                    channels
                } => {
                    assert_eq!(channels.len(), NT-1);

                    for chan in channels.into_iter() {
                        match chan {
                            Channel::Receiver { .. } => {},
                            Channel::Sender { .. } => { assert!(false); },
                        }
                    }
                },
                Data::Sender {..} => { assert!(false); },
            };
        }
    }

    #[test]
    fn test_make_si() {
        let data = make_channels(NT, Initiated::Sender);

        assert_eq!(data.len(), NT);

        for datum in data.into_iter() {
            match datum {
                Data::Sender {
                    channels, ..
                } => {
                    assert_eq!(channels.len(), NT-1);

                    for chan in channels.into_iter() {
                        match chan {
                            Channel::Sender { .. } => {},
                            Channel::Receiver { .. } => { assert!(false); },
                        }
                    }
                },
                Data::Receiver {..} => { assert!(false); },
            };
        }
    }

    macro_rules! tstcomm_reqs_ri {
        ($chan1:ident, $idx1:expr, $var:expr,
         $chan2:ident, $idx2:expr) => (
            match $chan1 {
                Data::Receiver {
                    ref channels,
                } => {
                    match channels[$idx1] {
                        Channel::Receiver {
                            ref request_send, ..
                        } => {
                            request_send.send().unwrap();
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };

            match $chan2 {
                Data::Receiver {
                    ref channels,
                } => {
                    match channels[$idx2] {
                        Channel::Receiver {
                            ref request_get, ..
                        } => {
                            let d = request_get.receive();
                            assert_eq!(d, $var);
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };
        );
    }

    macro_rules! tstcomm_reqs_si {
        ($chan1:ident, $var:expr,
         $chan2:ident, $idx2:expr) => (
            match $chan1 {
                Data::Sender {
                    ref request_send, ..
                } => {
                    request_send.send().unwrap();
                },
                _ => { assert!(false); },
            };

            match $chan2 {
                Data::Sender {
                    ref channels, ..
                } => {
                    match channels[$idx2] {
                        Channel::Sender {
                            ref request_get, ..
                        } => {
                            let d = request_get.receive();
                            assert_eq!(d, $var);
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };
        );
    }

    macro_rules! tstcomm_tasks_ri {
        ($chan1:ident, $idx1:expr, $var:expr,
         $chan2:ident, $idx2:expr) => (
            match $chan1 {
                Data::Receiver {
                    ref channels,
                } => {
                    match channels[$idx1] {
                        Channel::Receiver {
                            ref task_send, ..
                        } => {
                            task_send.send($var).unwrap();
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };

            match $chan2 {
                Data::Receiver {
                    ref channels,
                } => {
                    match channels[$idx2] {
                        Channel::Receiver {
                            ref task_get, ..
                        } => {
                            let d = task_get.try_receive().unwrap();
                            if let TaskData::OneTask(t) = d {
                                t.call_box();
                            }
                            else {
                                assert!(false);
                            }
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };
        );
    }

    macro_rules! tstcomm_tasks_si {
        ($chan1:ident, $idx1:expr, $var:expr,
         $chan2:ident, $idx2:expr) => (
            match $chan1 {
                Data::Sender {
                    ref channels, ..
                } => {
                    match channels[$idx1] {
                        Channel::Sender {
                            ref task_send, ..
                        } => {
                            task_send.send($var).unwrap();
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };

            match $chan2 {
                Data::Sender{
                    ref channels, ..
                } => {
                    match channels[$idx2] {
                        Channel::Sender {
                            ref task_get, ..
                        } => {
                            let d = task_get.try_receive().unwrap();
                            if let TaskData::OneTask(t) = d {
                                t.call_box();
                            }
                            else {
                                assert!(false);
                            }
                        },
                        _ => { assert!(false); },
                    };
                },
                _ => { assert!(false); },
            };
        );
    }

    #[test]
    fn test_requests_ri() {
        let mut data = make_channels(3, Initiated::Receiver);
        
        let chan3 = data.pop().unwrap();
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        tstcomm_reqs_ri!(chan1, 0, true, chan2, 0);
        tstcomm_reqs_ri!(chan1, 1, true, chan3, 0);
        tstcomm_reqs_ri!(chan2, 0, true, chan1, 0);
        tstcomm_reqs_ri!(chan2, 1, true, chan3, 1);
        tstcomm_reqs_ri!(chan3, 0, true, chan1, 1);
        tstcomm_reqs_ri!(chan3, 1, true, chan2, 1);
    }

    #[test]
    fn test_requests_si() {
        let mut data = make_channels(3, Initiated::Sender);
        let chan3 = data.pop().unwrap();
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        tstcomm_reqs_si!(chan1, true, chan2, 0);
        tstcomm_reqs_si!(chan1, true, chan3, 0);
        tstcomm_reqs_si!(chan2, true, chan1, 0);
        tstcomm_reqs_si!(chan2, true, chan3, 1);
        tstcomm_reqs_si!(chan3, true, chan1, 1);
        tstcomm_reqs_si!(chan3, true, chan2, 1);
    }

    #[test]
    fn test_tasks_ri() {
        use self::TaskData::OneTask as OT;
        
        let mut data = make_channels(3, Initiated::Receiver);
        let chan3 = data.pop().unwrap();
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        tstcomm_tasks_ri!(chan1, 0, OT(Box::new(|| {println!("Hello 1")})), chan2, 0);
        tstcomm_tasks_ri!(chan1, 1, OT(Box::new(|| {println!("Hello 2")})), chan3, 0);
        tstcomm_tasks_ri!(chan2, 0, OT(Box::new(|| {println!("Hello 3")})), chan1, 0);
        tstcomm_tasks_ri!(chan2, 1, OT(Box::new(|| {println!("Hello 4")})), chan3, 1);
        tstcomm_tasks_ri!(chan3, 0, OT(Box::new(|| {println!("Hello 5")})), chan1, 1);
        tstcomm_tasks_ri!(chan3, 1, OT(Box::new(|| {println!("Hello 6")})), chan2, 1);
    }

    #[test]
    fn test_tasks_si() {
        use self::TaskData::OneTask as OT;

        let mut data = make_channels(3, Initiated::Sender);
        let chan3 = data.pop().unwrap();
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        tstcomm_tasks_si!(chan1, 0, OT(Box::new(|| {println!("Hello 1")})), chan2, 0);
        tstcomm_tasks_si!(chan1, 1, OT(Box::new(|| {println!("Hello 2")})), chan3, 0);
        tstcomm_tasks_si!(chan2, 0, OT(Box::new(|| {println!("Hello 3")})), chan1, 0);
        tstcomm_tasks_si!(chan2, 1, OT(Box::new(|| {println!("Hello 4")})), chan3, 1);
        tstcomm_tasks_si!(chan3, 0, OT(Box::new(|| {println!("Hello 5")})), chan1, 1);
        tstcomm_tasks_si!(chan3, 1, OT(Box::new(|| {println!("Hello 6")})), chan2, 1);
    }
}
