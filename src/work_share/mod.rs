use std::collections::VecDeque;
use std::time::Duration;

use super::Task;

#[derive(Clone, Copy)]
pub enum ShareStrategy {
    One,
    Half,
}

#[derive(Clone, Copy)]
pub enum ReceiverWaitStrategy {
    Yield,
    Sleep(Duration),
}

pub enum TaskData {
    NoTasks,
    OneTask(Box<Task>),
    ManyTasks(VecDeque<Box<Task>>),
}

pub mod receiver_initiated;
pub mod sender_initiated;
