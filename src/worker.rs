use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc;

use rand;
use rand::{Rng, SeedableRng};

use super::{ShareStrategy, ReceiverWaitStrategy};
use super::boolchan as bc;
use super::channel::Data as ChannelData;
use super::channel::Channel;
use super::task::Task;
use super::task::Data as TaskData;
use super::taskchan as tc;

pub struct Config {
    pub index:            usize,
    pub task_capacity:    usize,
    pub share_strategy:   ShareStrategy,
    pub wait_strategy:  ReceiverWaitStrategy,
    pub channel_data:     ChannelData,
}

pub struct Worker {
    pub index:      usize,
    channel_length: usize,
    share_strategy: ShareStrategy,
    wait_strategy:  ReceiverWaitStrategy,
    tasks:          RefCell<VecDeque<Task>>,
    rng:            RefCell<rand::XorShiftRng>,
    channel_data:   ChannelData,
}

impl Worker {
    pub fn new(config: Config) -> Worker {
        let Config { index,
                     task_capacity: capacity,
                     share_strategy: share_strategy,
                     wait_strategy: wait_strategy,
                     channel_data: channel_data,
        } = config;

        let len = match channel_data {
            ChannelData::Receiver { ref channels } => { channels.len() },
            ChannelData::Sender { ref channels, .. } => { channels.len() },
        };

        Worker {
            index: index,
            channel_length: len,
            share_strategy: share_strategy,
            wait_strategy: wait_strategy,
            tasks: RefCell::new(VecDeque::with_capacity(capacity)),
            rng: RefCell::new(rand::XorShiftRng::from_seed([
                0, 0, 0, (index+1) as u32 // The seed cannot be all 0's
            ])),
            channel_data: channel_data,
        }
    }

    pub fn run(&self) {
        loop {
            self.run_once();
        }
    }

    #[inline]
    pub fn run_once(&self) {
        if self.tasks.borrow().is_empty() {
            self.acquire_tasks();
        }
        else {
            self.execute(self.tasks.borrow_mut().pop_front().unwrap());
            self.process_requests();
        }
    }

    #[inline]
    pub fn add_tasks(&self, task_data: TaskData) -> bool {
        match task_data {
            TaskData::ManyTasks(tasks) => {
                self.tasks.borrow_mut().append(&mut tasks);
                true
            },
            TaskData::OneTask(task) => {
                self.tasks.borrow_mut().push_back(task);
                true
            },
            TaskData::NoTasks => { false },
        }
    }

    #[inline]
    fn execute(&self, task: Task) {
        task.call_box();
    }

    fn try_get_tasks_ri(&self,
                        request_send: bc::Sender,
                        task_get: tc::Receiver) -> bool {
        
    }

    fn acquire_tasks(&self) {
        let mut got_tasks = false;
        
        match self.channel_data {
            ChannelData::Receiver {
                ref channels, ..
            } => {
                while !got_tasks {
                    let ridx = self.rand_index();
                    
                    match channels[ridx] {
                        Channel::Receiver {
                            ref request_send,
                            ref task_get,
                            ..
                        } => {
                            // Send request.
                            request_send.send().unwrap();

                            // Wait for either tasks or timeout.
                            match task_get.receive() {
                                Ok(data) => {
                                    if self.add_tasks(data) == true {
                                        got_tasks = true;
                                    }
                                },
                                Err(tc::ReceiveError::Timeout) => {
                                    // Try to 
                                    match request_send.try_unsend() {
                                        Err(bc::TryUnsendError::TooLate) => {
                                            if self.add_tasks(
                                                task_get.receive_no_timeout()
                                            ) == true {
                                                got_tasks = true;
                                            };
                                        },
                                    }
                                },
                            }
                        },
                        _ => {},
                    }
                }
            },
            Requests::Sender {
                ref send, ..
            } => {
                send.send(true).unwrap();

                // Loop waiting for other workers to send tasks
                let mut ridx = 0;

                while !got_tasks {
                    let num_tasks = match self.channels.responses_get[ridx].try_recv() {
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
                        self.tasks.borrow_mut().push_back(self.channels
                                                          .tasks_get[ridx]
                                                          .recv().unwrap());
                        got_tasks = true;
                    }
                }
            },
        }
    }

    fn process_requests(&self) {
        match self.channels.requests {
            Requests::Receiver {
                ref get, ..
            } => {
                for index in 0..self.channel_length {
                    // Do not trying sharing tasks if we do not have any.
                    if self.tasks.borrow().is_empty() {
                        break;
                    }
                    
                    if let Ok(_) = get[index].try_recv() {
                        self.share(index);
                    }
                }
            },
            Requests::Sender {
                ref get, ..
            } => {
                for index in 0..self.channel_length {
                    // Do not trying sharing tasks if we do not have any.
                    if self.tasks.borrow().is_empty() {
                        break;
                    }

                    if let Ok(_) = get[index].try_recv() {
                        self.share(index);
                    }
                }
            },
        }
    }

    fn share(&self, idx: usize) {
        match self.share_strategy {
            ShareStrategy::ONE => {
                self.channels.responses_send[idx].send(1).unwrap();
                self.channels.tasks_send[idx].send(self.tasks.borrow_mut()
                                                   .pop_front().unwrap()).unwrap();
            },
            ShareStrategy::HALF => {
                let half_len = self.tasks.borrow().len() / 2;

                self.channels.responses_send[idx].send(half_len).unwrap();

                if half_len > 0 {
                    for task in self.tasks.borrow_mut().split_off(half_len) {
                        self.channels.tasks_send[idx].send(task).unwrap();
                    }
                }
            },
        }
    }

    fn rand_index(&self) -> usize {
        (self.rng.borrow_mut().next_u32() as usize) % self.channel_length
    }
}

#[cfg(test)]
mod tests {
    use super::{Worker, Config};
    use super::super::{Initiated, ShareStrategy};
    use super::super::channel::make_channels;

    fn helper(initiated: Initiated,
              share: ShareStrategy) -> (Worker, Worker) {
        let mut channels = make_channels(2, initiated);
        
        let worker1 = Worker::new(Config {
            index: 0,
            task_capacity: 16,
            share_strategy: share,
            channel_data: channels.remove(0),
        });
        
        let worker2 = Worker::new(Config {
            index: 0,
            task_capacity: 16,
            share_strategy: share,
            channel_data: channels.remove(0),
        });

        (worker1, worker2)
    }

    #[test]
    fn test_make_worker_ri() {
        let mut channels = make_channels(2, Initiated::RECEIVER);
        #[allow(unused_variables)]
        let worker = Worker::new(Config {
            index: 0,
            task_capacity: 16,
            share_strategy: ShareStrategy::ONE,
            channel_data: channels.remove(0),
        });
    }

    #[test]
    fn test_make_worker_si() {
        let mut channels = make_channels(2, Initiated::SENDER);
        #[allow(unused_variables)]
        let worker = Worker::new(Config {
            index: 0,
            task_capacity: 16,
            share_strategy: ShareStrategy::ONE,
            channel_data: channels.remove(0),
        });
    }

    #[test]
    fn test_worker_addtask() {
        let (worker, _) = helper(Initiated::RECEIVER, ShareStrategy::ONE);

        worker.add_task(Box::new(|| { println!("Hello World!");}));

        assert!(worker.tasks.borrow_mut().len() == 1);
    }
}
