use itertools;

use super::super::channel;
use self::channel::boolean as bc;
use self::channel::task as tc;

pub struct Channel {
    pub request_get: bc::Receiver,
    pub task_send: tc::Sender,
    pub task_get: tc::Receiver,
}

pub struct Data {
    pub request_send: bc::Sender,
    pub channels: Vec<Channel>,
}

pub fn make_channels(nt: usize) -> Vec<Data> {
    let (tasks_tx, tasks_rx) = channel::make_shared_channels(nt);
    let (rqsts_tx, rqsts_rx) = make_si_channels(nt);

    itertools::multizip(
        (rqsts_tx, rqsts_rx, tasks_tx, tasks_rx)
    ).map(|(request_send, requests_get,
            tasks_send, tasks_get)| {
        Data {
            request_send: request_send,
            channels: itertools::multizip((requests_get,
                                           tasks_send,
                                           tasks_get))
                .map(|(request_get, task_send, task_get)| {
                    Channel {
                        request_get: request_get,
                        task_send: task_send,
                        task_get: task_get,
                    }
                }).collect::<Vec<Channel>>(),
        }
    }).collect::<Vec<Data>>()
}

fn make_si_channels(nt: usize)
                    -> (Vec<bc::Sender>,
                        Vec<Vec<bc::Receiver>>)
{
    let mut rqst_tx = Vec::<bc::Sender>::with_capacity(nt);
    let mut rqst_rx = Vec::<bc::Receiver>::with_capacity(nt);

    // Create initial channels.
    #[allow(unused_variables)]
    for n in 0..nt {
        let (tx, rx) = bc::channel();
        rqst_tx.push(tx);
        rqst_rx.push(rx);
    }

    // Clone Receivers N-2 times so there is one Receiver to give
    // to each of the OTHER workers.
    let mut rqst_rx_tmp = Vec::<Vec<bc::Receiver>>::with_capacity(nt);

    #[allow(unused_variables)]
    for n in 0..nt {
        let mut clones = Vec::<bc::Receiver>::with_capacity(nt-1);
        let rx = rqst_rx.remove(0);
        
        for nn in 0..(nt-2) {
            clones.push(rx.clone());
        }
        
        clones.push(rx);
        rqst_rx_tmp.push(clones);
    }

    // Swap out the receivers so each worker has a receiver for every OTHER worker.
    let mut rqst_rx = Vec::<Vec<bc::Receiver>>::with_capacity(nt);

    #[allow(unused_variables)]
    for n in 0..nt {
        let mut recvs = Vec::<bc::Receiver>::with_capacity(nt-1);

        for nn in 0..nt {
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
#[cfg(test)]
mod tests {
    use super::super::super::super::task::Data as TaskData;
    use super::*;

    static NT: usize = 3;

    #[test]
    fn test_make_si_channels() {
        let (rqst_tx, rqst_rx) = make_si_channels(NT);

        assert_eq!(rqst_tx.len(), NT);
        assert_eq!(rqst_rx.len(), NT);

        for v in rqst_rx.iter() {
            assert_eq!(v.len(), NT-1);
        }
    }

    #[test]
    fn test_make_channels() {
        let data = make_channels(NT);

        assert_eq!(data.len(), NT);

        for datum in data.into_iter() {
            assert_eq!(datum.channels.len(), NT-1);
        }
    }

    macro_rules! tstcomm_reqs_si {
        ($chan1:ident, $var:expr,
         $chan2:ident, $idx2:expr) => (
            $chan1.request_send.send().unwrap();
            let d = $chan2.channels[$idx2].request_get.receive();
            assert_eq!(d, $var);
        );
    }

    macro_rules! tstcomm_tasks_si {
        ($chan1:ident, $idx1:expr, $var:expr,
         $chan2:ident, $idx2:expr) => (
            $chan1.channels[$idx1].task_send.send(TaskData::OneTask($var)).unwrap();
            let d = $chan2.channels[$idx2].task_get.receive();
            if let TaskData::OneTask(task) = d {
                task.call_box();
            }
            else {
                assert!(false);
            }
        );
    }

    #[test]
    fn test_requests() {
        let mut data = make_channels(NT);
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
    fn test_tasks() {
        let mut data = make_channels(NT);
        let chan3 = data.pop().unwrap();
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        tstcomm_tasks_si!(chan1, 0, Box::new(|| {println!("Hello 1")}), chan2, 0);
        tstcomm_tasks_si!(chan1, 1, Box::new(|| {println!("Hello 2")}), chan3, 0);
        tstcomm_tasks_si!(chan2, 0, Box::new(|| {println!("Hello 3")}), chan1, 0);
        tstcomm_tasks_si!(chan2, 1, Box::new(|| {println!("Hello 4")}), chan3, 1);
        tstcomm_tasks_si!(chan3, 0, Box::new(|| {println!("Hello 5")}), chan1, 1);
        tstcomm_tasks_si!(chan3, 1, Box::new(|| {println!("Hello 6")}), chan2, 1);
    }
}
