use super::task::Task;

pub trait Worker {
    fn get_index(&self) -> usize; 
    fn signal_exit(&self); 
    fn add_task(&self, task: Task);
}
