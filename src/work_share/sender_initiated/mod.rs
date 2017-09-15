mod shared;
mod worker;
// pub mod pool;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use reqchan::{self, Requester, Responder};

use self::super::{ReceiverWaitStrategy, ShareStrategy, TaskData};
use self::shared::Data as SharedData;
use self::worker::Worker;
use self::worker::Config as WorkerConfig;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    use super::super::TaskData;
    
    use super::*;

    macro_rules! tstchan {
        ($req:ident, $resp:ident, $respnum:expr,
         $task:expr, $var:ident, $val:expr) => (
            {
                let mut reqcon = $req.try_request().unwrap();
                $resp[$respnum].try_respond().unwrap()
                    .send(TaskData::OneTask($task));
                if let Ok(TaskData::OneTask(t)) = reqcon.try_receive() {
                    t.call_box();
                }
                else {
                    assert!(false);
                }

                assert_eq!($var.load(Ordering::SeqCst), $val);
                $var.store(0, Ordering::SeqCst);
            }
        )
    }
  
    #[test]
    fn test_make_channels() {
        let var = Arc::new(AtomicUsize::new(0));
        let var1 = var.clone();
        let var2 = var.clone();
        let var3 = var.clone();
        let var4 = var.clone();
        let var5 = var.clone();
        let var6 = var.clone();
        
        let mut channels = make_channels(3);
        
        assert_eq!(channels.len(), 3);

        let (req2, resps2) = channels.pop().unwrap();

        assert_eq!(resps2.len(), 2);

        let (req1, resps1) = channels.pop().unwrap();

        assert_eq!(resps1.len(), 2);

        let (req0, resps0) = channels.pop().unwrap();

        assert_eq!(resps0.len(), 2);
        
        tstchan!(req0, resps1, 0,
                 Box::new(move || { var1.store(1, Ordering::SeqCst); }),
                 var, 1);
        tstchan!(req0, resps2, 0,
                 Box::new(move || { var2.store(2, Ordering::SeqCst); }),
                 var, 2);
        tstchan!(req1, resps0, 0,
                 Box::new(move || { var3.store(3, Ordering::SeqCst); }),
                 var, 3);
        tstchan!(req1, resps2, 1,
                 Box::new(move || { var4.store(4, Ordering::SeqCst); }),
                 var, 4);
        tstchan!(req2, resps0, 1,
                 Box::new(move || { var5.store(5, Ordering::SeqCst); }),
                 var, 5);
        tstchan!(req2, resps1, 1,
                 Box::new(move || { var6.store(6, Ordering::SeqCst); }),
                 var, 6);
    }
}
