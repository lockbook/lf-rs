#[cfg(feature = "mpsc")]
pub mod mpsc;

// Transport
pub trait Sender: Sized {
    type Message;
    type Error;

    fn send(&self, message: Self::Message) -> Result<(), SendError<Self::Message, Self::Error>>;
}

pub enum SendError<U, I> {
    /// The receiver has become disconnected and will receive no more data. The error contains the data being sent as a
    /// payload so it can be recovered.
    Disconnected(U),

    Inner(I),
}

pub trait Receiver<U>: Sized {
    type TryRecvError;
    type RecvError;

    fn try_recv(&mut self) -> Result<U, TryRecvError<Self::TryRecvError>>;
    fn recv(&mut self) -> Result<U, RecvError<Self::RecvError>>;
}

pub enum TryRecvError<I> {
    /// This receiver is currently empty, but the sender has not yet disconnected, so data may yet become available.
    Empty,

    /// The sender has become disconnected and will send no more data.
    Disconnected,

    Inner(I),
}

pub enum RecvError<I> {
    /// The sender has become disconnected and will send no more data.
    Disconnected,

    Inner(I),
}
