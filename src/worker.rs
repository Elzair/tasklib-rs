
pub trait Worker {
    fn get_index(&self) -> usize; 
    fn signal_exit(&self); 
}
