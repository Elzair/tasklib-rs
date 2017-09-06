use std::collections::VecDeque;

use super::super::task::Task;

pub enum Data {
    NoTasks,
    // OneTask(Task),
    // ManyTasks(VecDeque<Task>),
    OneTask(Box<Task>),
    ManyTasks(VecDeque<Box<Task>>),
}
