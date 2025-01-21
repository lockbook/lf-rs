use std::{ops::Deref, sync::mpsc};

// State
#[derive(Clone, Debug)]
pub struct State<S> {
    snapshot: S,
    seq: u64,
}

// Update
pub trait Update<S>: Sized {
    fn apply(&self, target: &mut S);
}

impl<S> Update<S> for S
where
    S: Copy,
{
    fn apply(&self, target: &mut S) {
        *target = *self;
    }
}

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

// Leader
#[derive(Debug)]
pub struct Leader<S, U: Update<S>, Tx: Sender<(U, u64)>> {
    state: State<S>,
    followers: Vec<Tx>,
    _update: std::marker::PhantomData<U>,
}

impl<S, U: Update<S>, Tx: Sender<(U, u64)>> Deref for Leader<S, U, Tx> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state.snapshot
    }
}

impl<S: Clone, U: Update<S> + Clone, Tx: Sender<(U, u64)>> Leader<S, U, Tx> {
    pub fn new(state: S) -> Self {
        Self {
            state: State {
                snapshot: state,
                seq: 0,
            },
            followers: Vec::new(),
            _update: std::marker::PhantomData,
        }
    }

    pub fn update(&mut self, update: U) -> u64
    where
        U: Update<S>,
    {
        update.apply(&mut self.state.snapshot);
        self.state.seq += 1;

        for i in (0..self.followers.len()).rev() {
            if self.followers[i]
                .send((update.clone(), self.state.seq))
                .is_err()
            {
                self.followers.remove(i);
            }
        }

        self.state.seq
    }

    pub fn lead(&mut self, tx: Tx) {
        self.followers.push(tx);
    }

    pub fn seq(&self) -> u64 {
        self.state.seq
    }
}

// Follower
#[derive(Debug)]
pub struct Follower<S, U: Update<S>, Rx: Receiver<(U, u64)>> {
    state: State<S>,
    leader: Rx,
    waiters: Vec<(pulse::Pulse, u64)>,
    _update: std::marker::PhantomData<U>,
}

// todo: feature gate or something
impl<S: Clone, U: Update<S> + Clone> Follower<S, U, mpsc::Receiver<(U, u64)>> {
    pub fn follow(leader: &mut Leader<S, U, mpsc::Sender<(U, u64)>>) -> Self {
        let (tx, rx) = mpsc::channel();
        leader.lead(tx);
        Self {
            state: leader.state.clone(),
            leader: rx,
            waiters: Vec::new(),
            _update: std::marker::PhantomData,
        }
    }
}

impl<S: Clone, U: Update<S> + Clone, Rx: Receiver<(U, u64)>> Follower<S, U, Rx> {
    /// Applies all available updates.
    pub fn refresh(&mut self) -> <Rx as Receiver<(U, u64)>>::TryRecvError {
        loop {
            match self.leader.try_recv() {
                Ok((update, seq)) => {
                    self.apply(update, seq);
                }
                Err(e) => return e,
            }
        }
    }

    pub fn seq(&self) -> u64 {
        self.state.seq
    }

    /// Returns a pulse::Signal which can be used to wait for the follower to reach a certain sequence number in sync
    /// or async contexts.
    pub fn signal_at(&mut self, seq: u64) -> pulse::Signal {
        let (signal, pulse) = pulse::Signal::new();
        self.waiters.push((pulse, seq));
        signal
    }

    fn apply(&mut self, update: U, seq: u64) {
        update.apply(&mut self.state.snapshot);
        self.state.seq = seq;
    }
}

impl<S, U: Update<S>, Rx: Receiver<(U, u64)>> Deref for Follower<S, U, Rx> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state.snapshot
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let mut leader = Leader::new(420);
        let mut follower = Follower::follow(&mut leader);

        leader.update(69);

        let f: i32 = *follower;
        assert_eq!(f, 420);

        follower.refresh();

        let f: i32 = *follower;
        assert_eq!(f, 69);
    }

    // change notification: sequence numbers
    #[test]
    fn test_seq() {
        let mut leader = Leader::new(420);
        let mut follower = Follower::follow(&mut leader);

        assert_eq!(leader.seq(), 0);

        leader.update(69);

        assert_eq!(leader.seq(), 1);
        assert_eq!(follower.seq(), 0);

        follower.refresh();

        assert_eq!(follower.seq(), 1);
    }

    // change notification: pulse
    #[test]
    fn test_pulse() {
        let mut leader = Leader::new(420);
        let mut follower = Follower::follow(&mut leader);

        let seq = leader.update(69);
        let signal = follower.signal_at(seq);

        let f: i32 = *follower;
        assert_eq!(f, 420);

        follower.refresh();
        signal.wait().unwrap();

        let f: i32 = *follower;
        assert_eq!(f, 69);
    }
}
