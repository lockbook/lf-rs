use std::sync::mpsc;

use crate::{Receiver, Sender};

use super::{RecvError, SendError, TryRecvError};

impl<U> Sender for mpsc::Sender<U> {
    type Message = U;
    type Error = mpsc::SendError<U>;

    fn send(&self, update: U) -> Result<(), SendError<U, Self::Error>> {
        Ok(self.send(update)?)
    }
}

impl<U> Receiver<U> for mpsc::Receiver<U> {
    type TryRecvError = mpsc::TryRecvError;
    type RecvError = mpsc::RecvError;

    fn try_recv(&mut self) -> Result<U, TryRecvError<Self::TryRecvError>> {
        Ok(mpsc::Receiver::try_recv(self)?)
    }

    fn recv(&mut self) -> Result<U, RecvError<Self::RecvError>> {
        Ok(mpsc::Receiver::recv(self)?)
    }
}

impl<U> From<mpsc::SendError<U>> for SendError<U, mpsc::SendError<U>> {
    /// "A **send** operation can only fail if the receiving end of a channel is disconnected"
    fn from(value: mpsc::SendError<U>) -> Self {
        SendError::Disconnected(value.0)
    }
}

impl From<mpsc::TryRecvError> for TryRecvError<mpsc::TryRecvError> {
    fn from(value: mpsc::TryRecvError) -> Self {
        match value {
            mpsc::TryRecvError::Empty => TryRecvError::Empty,
            mpsc::TryRecvError::Disconnected => TryRecvError::Disconnected,
        }
    }
}

impl From<mpsc::RecvError> for RecvError<mpsc::RecvError> {
    /// The [`recv`] operation can only fail if the sending half of a [`channel`] (or [`sync_channel`]) is disconnected
    fn from(_: mpsc::RecvError) -> Self {
        RecvError::Disconnected
    }
}
