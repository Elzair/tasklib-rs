/// This trait allows a boxed closure to move itself out of its `Box`
/// in order to take ownership of itself.
pub trait FnBox {
    fn call_box(self: Box<Self>);
}

// To call an `FnOnce` closure stored in a `Box`, the closure needs to move
// itself out of the `Box` since it takes ownership of itself when called.
impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

pub type Task = Box<FnBox + Send + 'static>;

