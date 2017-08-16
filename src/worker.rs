use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc;

use rand;
use rand::{Rng,SeedableRng};

use super::ShareStrategy;
use task::Task;
use channel;

pub struct Config {
    pub index:            usize,
    pub task_capacity:    usize,
    pub share_strategy:   ShareStrategy,
    pub channel_data:     channel::Data,
}

pub struct Worker {
    pub index:      usize,
    channel_length: usize,
    share_strategy: ShareStrategy,
    tasks:          RefCell<VecDeque<Task>>,
    rng:            RefCell<rand::XorShiftRng>,
    channels:       channel::Data,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        let Config { index,
                     task_capacity: capacity,
                     share_strategy: share,
                     channel_data: channels
        } = config;

        let len = match channels {
            channel::Data::Receiver {
                ref get_requests,
                ..
            } => {
                get_requests.len()
            },
            channel::Data::Sender {
                ref get_requests,
                ..
            } => {
                get_requests.len()
            },
        };

        Worker {
            index: index,
            channel_length: len,
            share_strategy: share,
            tasks: RefCell::new(VecDeque::with_capacity(capacity)),
            rng: RefCell::new(rand::XorShiftRng::from_seed([
                0, 0, 0, (index+1) as u32 // The seed cannot be all 0's
            ])),
            channels: channels,
        }
    }

    pub fn run(&self) {
        loop {
            self.run_once();
        }
    }

    pub fn run_once(&self) {
        // Popping the task first ensures this Worker does not
        // give away its only task.
        match self.tasks.borrow_mut().pop_front() {
            Some(task) => {
                self.process_requests();
                self.execute(task);
            },
            None => {
                self.acquire_tasks();
            },
        }
    }

    pub fn add_task(&self, task: Task) {
        self.tasks.borrow_mut().push_back(task);
    }

    fn execute(&self, task: Task) {
        task.call_box();
    }

    fn acquire_tasks(&self) {
        let mut got_tasks = false;
        
        #[allow(unused_variables)]
        match self.channels {
            channel::Data::Receiver {
                ref send_requests,
                ref get_responses,
                ref get_tasks,
                ..
            } => {
                while !got_tasks {
                    let ridx = self.rand_index();
                    
                    send_requests[ridx].send(true).unwrap();

                    let num_tasks = get_responses[ridx].recv().unwrap();

                    for n in 0..num_tasks {
                        self.tasks.borrow_mut() .push_back(get_tasks[ridx]
                                                           .recv().unwrap());
                        got_tasks = true;
                    }
                }
            },
            channel::Data::Sender {
                ref send_requests,
                ref get_responses,
                ref get_tasks,
                ..
            } => {
                send_requests.send(true).unwrap();

                // Loop waiting for other workers to send tasks
                let mut ridx = 0;

                while !got_tasks {
                    let num_tasks = match get_responses[ridx].try_recv() {
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
                        self.tasks.borrow_mut().push_back(get_tasks[ridx]
                                                          .recv().unwrap());
                        got_tasks = true;
                    }
                }
            },
        }
    }

    fn process_requests(&self) {
        match self.channels {
            channel::Data::Receiver {
                ref get_requests,
                ..
            } => {
                for index in 0..self.channel_length {
                    // Do not trying sharing tasks if we do not have any.
                    if self.tasks.borrow().len() == 0 {
                        break;
                    }

                    let idx = index % self.channel_length;
                    
                    if let Ok(_) = get_requests[idx].try_recv() {
                        self.share(idx);
                    }
                }
            },
            channel::Data::Sender {
                ref get_requests,
                ..
            } => {
                for index in 0..self.channel_length {
                    // Do not trying sharing tasks if we do not have any.
                    if self.tasks.borrow().len() == 0 {
                        break;
                    }

                    let idx = index % self.channel_length;

                    if let Ok(_) = get_requests[idx].try_recv() {
                        self.share(idx);
                    }
                }
            },
        }
    }

    fn share(&self, idx: usize) {
        match self.share_strategy {
            ShareStrategy::ONE => {
                match self.channels {
                    channel::Data::Receiver {
                        ref send_responses,
                        ref send_tasks,
                        ..
                    } => {
                        send_responses[idx].send(1).unwrap();
                        send_tasks[idx].send(self.tasks.borrow_mut()
                                             .pop_front().unwrap()).unwrap();
                    },
                    channel::Data::Sender {
                        ref send_responses,
                        ref send_tasks,
                        ..
                    } => {
                        send_responses[idx].send(1).unwrap();
                        send_tasks[idx].send(self.tasks.borrow_mut()
                                             .pop_front().unwrap()).unwrap();
                    },
                }
            },
            ShareStrategy::HALF => {
                let half_len = self.tasks.borrow().len() / 2;

                match self.channels {
                    channel::Data::Receiver {
                        ref send_responses,
                        ref send_tasks,
                        ..
                    } => {
                        send_responses[idx].send(half_len).unwrap();

                        for task in self.tasks.borrow_mut().split_off(half_len) {
                            send_tasks[idx].send(task).unwrap();
                        }
                    },
                    channel::Data::Sender {
                        ref send_responses,
                        ref send_tasks,
                        ..
                    } => {
                        send_responses[idx].send(half_len).unwrap();

                        for task in self.tasks.borrow_mut().split_off(half_len) {
                            send_tasks[idx].send(task).unwrap();
                        }
                    },
                }
            },
        }
    }

    fn rand_index(&self) -> usize {
        (self.rng.borrow_mut().next_u32() as usize) % self.channel_length
    }
}
