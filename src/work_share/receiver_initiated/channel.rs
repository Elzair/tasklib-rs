use itertools;

use super::super::util;
use super::super::channel;
use super::super::task::Data as TaskData;

use self::channel::boolean as bc;
use self::channel::task as tc;

pub struct Channel {
    pub request_send: bc::Sender,
    pub request_get: bc::Receiver,
    pub task_send: tc::Sender<TaskData>,
    pub task_get: tc::Receiver<TaskData>,
}

pub struct Data {
    pub channels: Vec<Channel>,
}

pub fn make_channels(nt: usize) -> Vec<Data> {
    let (tasks_tx, tasks_rx) = channel::make_shared_channels::<TaskData>(nt);
    let (rqsts_tx, rqsts_rx) = make_ri_channels(nt);

    itertools::multizip(
        (rqsts_tx, rqsts_rx, tasks_tx, tasks_rx)
    ).map(|(requests_send, requests_get,
            tasks_send, tasks_get)| {
        Data {
            channels: itertools::multizip((requests_send,
                                           requests_get,
                                           tasks_send,
                                           tasks_get))
                .map(|(request_send, request_get,
                       task_send, task_get)| {
                    Channel {
                        request_send: request_send,
                        request_get: request_get,
                        task_send: task_send,
                        task_get: task_get,
                    }
                }).collect::<Vec<Channel>>()
        }
    }).collect::<Vec<Data>>()
}

fn make_ri_channels(nt: usize)
                    -> (Vec<Vec<bc::Sender>>,
                        Vec<Vec<bc::Receiver>>)
{
    let ntsq = nt * nt;
    
    // Model the channels as several NxN matrices.
    let mut rqst_tx = Vec::<Option<bc::Sender>>::with_capacity(ntsq);
    let mut rqst_rx = Vec::<Option<bc::Receiver>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match util::is_diagonal(n, nt) {
            true => {
                rqst_tx.push(None);
                rqst_rx.push(None);
            },
            false => {
                let (tx, rx) = bc::channel();
                rqst_tx.push(Some(tx));
                rqst_rx.push(Some(rx));
            },
        }
    }

    // Give one part of each channel to its corresponding thread.
    // This is accomplished by transposing the `rqst_rx` 'matrix'.
    for n in 0..ntsq {
        if util::is_lower_left(n, nt) {
            rqst_rx.swap(n, util::get_transpose_index(n, nt));
        }
    }

    (
        util::split_vec(util::filter_vec(rqst_tx), nt, nt-1),
        util::split_vec(util::filter_vec(rqst_rx), nt, nt-1),
    )
}

#[cfg(test)]
mod tests {
    use super::super::super::task::Data as TaskData;
    use super::*;

    static NT: usize = 3;
    
    #[test]
    fn test_make_channels_ri() {
        let (rqst_tx, rqst_rx) = make_ri_channels(NT);

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
    fn test_make_channels() {
        let data = make_channels(NT);

        assert_eq!(data.len(), NT);

        for datum in data.into_iter() {
            assert_eq!(datum.channels.len(), NT-1);
        }
    }

    macro_rules! tstcomm_reqs_ri {
        ($chan1:ident, $idx1:expr, $var:expr,
         $chan2:ident, $idx2:expr) => (
            $chan1.channels[$idx1].request_send.send().unwrap();
            let d = $chan2.channels[$idx2].request_get.receive();
            assert_eq!(d, $var);
        );
    }

    macro_rules! tstcomm_tasks_ri {
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
        let mut data = make_channels(3);
        
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
    fn test_tasks() {
        let mut data = make_channels(3);
        let chan3 = data.pop().unwrap();
        let chan2 = data.pop().unwrap();
        let chan1 = data.pop().unwrap();

        tstcomm_tasks_ri!(chan1, 0, Box::new(|| {println!("Hello 1")}), chan2, 0);
        tstcomm_tasks_ri!(chan1, 1, Box::new(|| {println!("Hello 2")}), chan3, 0);
        tstcomm_tasks_ri!(chan2, 0, Box::new(|| {println!("Hello 3")}), chan1, 0);
        tstcomm_tasks_ri!(chan2, 1, Box::new(|| {println!("Hello 4")}), chan3, 1);
        tstcomm_tasks_ri!(chan3, 0, Box::new(|| {println!("Hello 5")}), chan1, 1);
        tstcomm_tasks_ri!(chan3, 1, Box::new(|| {println!("Hello 6")}), chan2, 1);
    }
}
