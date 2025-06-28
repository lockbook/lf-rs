use crate::{Lead, Receiver, RecvError, SendError, Sender, TryRecvError};

pub struct SendMap<I, O, F: Fn(&I) -> O, Tx: Sender<Message = O>> {
    f: F,
    tx: Option<Tx>,
    _u: std::marker::PhantomData<I>,
    _o: std::marker::PhantomData<O>,
}

impl<I, O, F: Fn(&I) -> O, Tx: Sender<Message = O>> SendMap<I, O, F, Tx> {
    pub fn new(f: F) -> Self {
        Self {
            f,
            tx: None,
            _u: std::marker::PhantomData,
            _o: std::marker::PhantomData,
        }
    }
}

impl<I, O, F: Fn(&I) -> O, Tx: Sender<Message = O>> Sender for SendMap<I, O, F, Tx> {
    type Message = I;
    type Error = Tx::Error;

    fn send(&self, input: I) -> Result<(), SendError<I, Self::Error>> {
        if let Some(tx) = &self.tx {
            let output = (self.f)(&input);
            match tx.send(output) {
                Ok(()) => Ok(()),
                Err(SendError::Disconnected(_)) => Err(SendError::Disconnected(input)),
                Err(SendError::Inner(e)) => Err(SendError::Inner(e)),
            }
        } else {
            Err(SendError::Disconnected(input))
        }
    }
}

impl<U, O, F: Fn(&U) -> O, Tx: Sender<Message = O>> Lead<U, O, F, Tx> for SendMap<U, O, F, Tx> {
    fn lead(&mut self, follower: SendMap<U, O, F, Tx>) -> &mut SendMap<U, O, F, Tx> {
        self.tx = Some(follower);
        self.follower.as_mut().unwrap()
    }
}

pub struct RecvMap<I, O, F: Fn(I) -> O, Rx: Receiver<I>> {
    f: F,
    rx: Option<Rx>,
    _u: std::marker::PhantomData<I>,
    _o: std::marker::PhantomData<O>,
}

impl<I, O, F: Fn(I) -> O, Rx: Receiver<I>> Receiver<O> for RecvMap<I, O, F, Rx> {
    type TryRecvError = Rx::TryRecvError;
    type RecvError = Rx::RecvError;

    fn try_recv(&mut self) -> Result<O, TryRecvError<Self::TryRecvError>> {
        if let Some(rx) = &mut self.rx {
            let input = rx.try_recv()?;
            Ok((self.f)(input))
        } else {
            Err(TryRecvError::Disconnected)
        }
    }

    fn recv(&mut self) -> Result<O, RecvError<Self::RecvError>> {
        if let Some(rx) = &mut self.rx {
            let input = rx.recv()?;
            Ok((self.f)(input))
        } else {
            Err(RecvError::Disconnected)
        }
    }
}
