use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc;

use spmc;
use rand;
use rand::{Rng,SeedableRng};

use super::ShareStrategy;
use task::Task;

pub enum InitiatedConfig {
    Receiver (
        Vec<(mpsc::Sender<bool>, mpsc::Receiver<bool>)>,
    ),
    Sender {
        send_requests: spmc::Sender<bool>,
        get_requests:  Vec<spmc::Receiver<bool>>,
    },
}

pub struct Config {
    pub index:            usize,
    pub task_capacity:    usize,
    pub share_strategy:   ShareStrategy,
    pub initiated_config: InitiatedConfig,
    pub channels:         Vec<(// mpsc::Sender<bool>,
                               // mpsc::Receiver<bool>,
                               mpsc::Sender<usize>,
                               mpsc::Receiver<usize>,
                               mpsc::Sender<Task>,
                               mpsc::Receiver<Task>)>,
}

enum InitiatedData {
    Receiver {
        send_requests: Vec<mpsc::Sender<bool>>,
        get_requests:  Vec<mpsc::Receiver<bool>>,
    },
    Sender {
        send_requests: spmc::Sender<bool>,
        get_requests:  Vec<spmc::Receiver<bool>>,
    }
}

pub struct Worker {
    pub index:      usize,
    channel_length: usize,
    start_idx:      usize,
    share_strategy: ShareStrategy,
    tasks:          VecDeque<Task>,
    // rng:            rand::XorShiftRng,
    rng:            RefCell<rand::XorShiftRng>,
    initiated_data: InitiatedData,
    // send_requests:  Vec<mpsc::Sender<bool>>,
    // get_requests:   Vec<mpsc::Receiver<bool>>,
    send_responses: Vec<mpsc::Sender<usize>>,
    get_responses:  Vec<mpsc::Receiver<usize>>,
    send_tasks:     Vec<mpsc::Sender<Task>>,
    get_tasks:      Vec<mpsc::Receiver<Task>>,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        let Config { index: index,
                     task_capacity: capacity,
                     share_strategy: share,
                     initiated_config: initcfg,
                     channels: chans
        } = config;
        // let (index, capacity, share, chans) = (config.index,
        //                                        config.task_capacity,
        //                                        config.share_strategy,
        //                                        config.channels);

        // Process configuration for config-specific channels.
        let initiated_data = match initcfg {
            InitiatedConfig::Receiver(chans) => {

                let len = chans.len();
                let mut send_requests = Vec::<mpsc::Sender<bool>>::with_capacity(len);
                let mut get_requests = Vec::<mpsc::Receiver<bool>>::with_capacity(len);

                for (rqst_tx, rqst_rx) in chans.into_iter() {

                    send_requests.push(rqst_tx);
                    get_requests.push(rqst_rx);
                }

                InitiatedData::Receiver {
                    send_requests: send_requests,
                    get_requests: get_requests,
                }
            },
            InitiatedConfig::Sender{ send_requests, get_requests } => {
                InitiatedData::Sender {
                    send_requests: send_requests,
                    get_requests: get_requests,
                }
            },
        };

        // Process configuration for normal channels.
        let len = chans.len();

        // let mut send_requests = Vec::<mpsc::Sender<bool>>::with_capacity(len);
        // let mut get_requests = Vec::<mpsc::Receiver<bool>>::with_capacity(len);
        let mut send_responses = Vec::<mpsc::Sender<usize>>::with_capacity(len);
        let mut get_responses = Vec::<mpsc::Receiver<usize>>::with_capacity(len);
        let mut send_tasks = Vec::<mpsc::Sender<Task>>::with_capacity(len);
        let mut get_tasks = Vec::<mpsc::Receiver<Task>>::with_capacity(len);

        for (// rqst_tx, rqst_rx,
             resp_tx, resp_rx,
             jobs_tx, jobs_rx) in chans.into_iter() {
            // send_requests.push(rqst_tx);
            // get_requests.push(rqst_rx);
            send_responses.push(resp_tx);
            get_responses.push(resp_rx);
            send_tasks.push(jobs_tx);
            get_tasks.push(jobs_rx);
        }

        Worker {
            index: index,
            channel_length: len,
            start_idx: 0,
            share_strategy: share,
            tasks: VecDeque::with_capacity(capacity),
            // rng: rand::XorShiftRng::from_seed([
            //     0, 0, 0, (index+1) as u32 // The seed cannot be all 0's
            // ]),
            rng: RefCell::new(rand::XorShiftRng::from_seed([
                0, 0, 0, (index+1) as u32 // The seed cannot be all 0's
            ])),
            initiated_data: initiated_data,
            // send_requests: send_requests,  
            // get_requests: get_requests,  
            send_responses: send_responses,  
            get_responses: get_responses,  
            send_tasks: send_tasks,
            get_tasks: get_tasks,
        }
    }

    pub fn run(&mut self) {
        loop {
            self.run_once();
        }
    }

    pub fn run_once(&mut self) {
        // Popping the task first ensures this Worker does not
        // give away its only task.
        match self.tasks.pop_front() {
            Some(task) => {
                self.process_requests();
                self.execute(task);
            },
            None => {
                self.acquire_tasks();
            },
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push_back(task);
    }

    fn execute(&self, task: Task) {
        task.call_box();
    }

    fn acquire_tasks(&mut self) {
        let mut got_tasks = false;
        
        // while !got_tasks {
        //     let ridx = self.rand_index();
            
        //     // self.send_requests[ridx].send(true).unwrap();

        //     let num_jobs = self.get_responses[ridx].recv().unwrap();

        //     #[allow(unused_variables)]
        //     for n in 0..num_jobs {
        //         self.tasks.push_back(self.get_tasks[ridx].recv().unwrap());
        //         got_tasks = true;
        //     }
        // }
        #[allow(unused_variables)]
        match self.initiated_data {
            InitiatedData::Receiver { ref send_requests, ref get_requests } => {
                while !got_tasks {
                    let ridx = self.rand_index();
                    
                    send_requests[ridx].send(true).unwrap();

                    let num_tasks = self.get_responses[ridx].recv().unwrap();

                    for n in 0..num_tasks {
                        self.tasks.push_back(self.get_tasks[ridx].recv().unwrap());
                        got_tasks = true;
                    }
                }
            },
            InitiatedData::Sender { ref send_requests, ref get_requests } => {
                send_requests.send(true).unwrap();

                // Loop waiting for other workers to send tasks
                let mut ridx = 0;

                while !got_tasks {
                    let num_tasks = match self.get_responses[ridx].try_recv() {
                        Ok(nt) => nt,
                        Err(mpsc::TryRecvError::Empty) => {
                            ridx = (ridx + 1) % self.channel_length;
                            continue;
                        },
                        // TODO: Do something else instead of panicking.
                        _ => {
                            panic!("Sender panicked!");
                        }
                    };

                    for n in 0..num_tasks {
                        self.tasks.push_back(self.get_tasks[ridx].recv().unwrap());
                        got_tasks = true;
                    }
                }
            },
        }
    }

    fn process_requests(&mut self) {
        for index in self.start_idx..(self.start_idx + self.channel_length) {
            // Do not trying sharing tasks if we do not have any.
            if self.tasks.len() == 0 {
                break;
            }

            let idx = index % self.channel_length;
            
            // if let Ok(_) = self.get_requests[idx].try_recv() {
            //     self.share(idx);
            // }
        }

        // Start from a different index so the same workers
        // do not get dibs on tasks.
        self.start_idx = (self.start_idx + 1) % self.channel_length;
    }

    fn share(&mut self, idx: usize) {
        match self.share_strategy {
            ShareStrategy::ONE => {
                self.send_responses[idx].send(1).unwrap();
                self.send_tasks[idx].send(self.tasks.pop_front().unwrap()).unwrap();
            },
            ShareStrategy::HALF => {
                let half_len = self.tasks.len() / 2;
                self.send_responses[idx].send(half_len).unwrap();

                for task in self.tasks.split_off(half_len) {
                    self.send_tasks[idx].send(task).unwrap();
                }
            },
        }
    }

    fn rand_index(&self) -> usize {
        (self.rng.borrow_mut().next_u32() as usize) % self.channel_length
    }
}
