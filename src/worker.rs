use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;
use std::thread;

use rand;
use rand::{Rng,SeedableRng};

use task::Task;

pub struct Config {
    pub index:         usize,
    pub task_capacity: usize,
    pub channels:      Vec<(mpsc::Sender<bool>,
                            mpsc::Receiver<bool>,
                            mpsc::Sender<bool>,
                            mpsc::Receiver<bool>,
                            mpsc::Sender<Task>,
                            mpsc::Receiver<Task>)>,
}

pub struct Worker {
    index:          usize,
    tasks:          RefCell<VecDeque<Task>>,
    rng:            RefCell<rand::XorShiftRng>,
    send_requests:  Vec<mpsc::Sender<bool>>,
    get_requests:   Vec<mpsc::Receiver<bool>>,
    send_responses: Vec<mpsc::Sender<bool>>,
    get_responses:  Vec<mpsc::Receiver<bool>>,
    send_tasks:     Vec<mpsc::Sender<Task>>,
    get_tasks:      Vec<mpsc::Receiver<Task>>,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        let (index, capacity, chans) = (config.index,
                                        config.task_capacity,
                                        config.channels);

        let mut send_requests = Vec::<mpsc::Sender<bool>>::with_capacity(chans.len());
        let mut get_requests = Vec::<mpsc::Receiver<bool>>::with_capacity(chans.len());
        let mut send_responses = Vec::<mpsc::Sender<bool>>::with_capacity(chans.len());
        let mut get_responses = Vec::<mpsc::Receiver<bool>>::with_capacity(chans.len());
        let mut send_tasks = Vec::<mpsc::Sender<Task>>::with_capacity(chans.len());
        let mut get_tasks = Vec::<mpsc::Receiver<Task>>::with_capacity(chans.len());

        for (rqst_tx, rqst_rx,
             resp_tx, resp_rx,
             jobs_tx, jobs_rx) in chans.into_iter() {
            send_requests.push(rqst_tx);
            get_requests.push(rqst_rx);
            send_responses.push(resp_tx);
            get_responses.push(resp_rx);
            send_tasks.push(jobs_tx);
            get_tasks.push(jobs_rx);
        }

        Worker {
            index:          index,
            tasks:          RefCell::new(VecDeque::with_capacity(capacity)),
            rng:            RefCell::new(rand::XorShiftRng::from_seed(
                            [0, 0, 0, (index+1) as u32])),
            send_requests:  send_requests,  
            get_requests:   get_requests,  
            send_responses: send_responses,  
            get_responses:  get_responses,  
            send_tasks:     send_tasks,
            get_tasks:      get_tasks,
        }
    }

    // pub fn main(&self) {
    //     loop {
    //         match self.tasks.borrow_mut().pop_front() {
    //             Some(task) => {
    //                 // Popping the task first ensures this Worker does not
    //                 // give away its only task.
    //                 // self.process_share_requests();
    //                 // self.execute(task);
    //             },
    //             None => {
    //                 self.tasks.borrow_mut().push_back(self.acquire_task());
    //             },
    //         }
    //     }
    // }

    // fn acquire_task(&self) -> Task {
    //     loop {
    //         let idx = (self.rng.borrow_mut().next_u32() as usize) % self.requests.len();

    //         if idx == self.index {
    //             continue;
    //         }
            
    //     }
    // }
}
