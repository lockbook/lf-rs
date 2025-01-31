use std::sync::mpsc;

// Transport
pub trait Sender<U>: Sized {
    type SendError;

    fn send(&self, update: U) -> Result<(), Self::SendError>;
}

pub trait Receiver<U> {
    type TryRecvError;
    type RecvError;

    fn try_recv(&self) -> Result<U, Self::TryRecvError>;
    fn recv(&self) -> Result<U, Self::RecvError>;
}

// todo: feature gate or something
impl<U> Sender<U> for mpsc::Sender<U> {
    type SendError = mpsc::SendError<U>;

    fn send(&self, update: U) -> Result<(), Self::SendError> {
        self.send(update)
    }
}

impl<U> Receiver<U> for mpsc::Receiver<U> {
    type TryRecvError = mpsc::TryRecvError;
    type RecvError = mpsc::RecvError;

    fn try_recv(&self) -> Result<U, Self::TryRecvError> {
        self.try_recv()
    }

    fn recv(&self) -> Result<U, Self::RecvError> {
        self.recv()
    }
}
