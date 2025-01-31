use std::sync::mpsc;

/// The `Update` trait represets a deterministic state transition for another type. Implement this trait to unlock the
/// features of this crate.
pub trait Update<S>: Sized {
    fn apply(&self, target: &mut S);
}

// All copy types are updates of themselves. The update is simply to overwrite the value.
impl<S> Update<S> for S
where
    S: Copy,
{
    fn apply(&self, target: &mut S) {
        *target = *self;
    }
}

// Sequenced data has changes that occur at ordered sequence numbers.
#[derive(Clone, Debug)]
pub struct Sequenced<S> {
    value: S,
    seq: u64,
}

impl<S> Sequenced<S> {
    pub fn new(value: S) -> Self {
        Self { value, seq: 0 }
    }
}

// Sequenced updates are updates for sequenced data. The sequence number of the sequenced data will be updated to the
// sequence number of the sequenced update. Such are the updates to the sequence of sequenced updates.
impl<S, U> Update<Sequenced<S>> for Sequenced<U>
where
    U: Update<S>,
{
    fn apply(&self, target: &mut Sequenced<S>) {
        self.value.apply(&mut target.value);
        target.seq = self.seq;
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

// Leader
#[derive(Debug)]
pub struct Leader<S, U: Update<S>, Tx: Sender<U>> {
    state: S,
    follower: Tx,
    _update: std::marker::PhantomData<U>,
}

impl<S: Clone, U: Update<S>, Tx: Sender<U>> Leader<S, U, Tx> {
    pub fn new(state: S, follower: Tx) -> Self {
        Self {
            state,
            follower,
            _update: std::marker::PhantomData,
        }
    }

    pub fn update(&mut self, update: U) -> Result<(), Tx::SendError>
    where
        U: Update<S>,
    {
        update.apply(&mut self.state);

        self.follower.send(update)
    }
}

impl<S: Clone, U: Update<S>, Tx: Sender<Sequenced<U>>> Leader<Sequenced<S>, Sequenced<U>, Tx> {
    pub fn seq(&self) -> u64 {
        self.state.seq
    }

    pub fn sequenced_update(&mut self, update: U) -> Result<(), Tx::SendError> {
        self.state.seq += 1;
        let sequenced_update = Sequenced {
            value: update,
            seq: self.state.seq,
        };

        self.update(sequenced_update)
    }
}

// Follower
#[derive(Debug)]
pub struct Follower<S, U: Update<S>, Rx: Receiver<U>> {
    state: S,
    leader: Rx,
    _update: std::marker::PhantomData<U>,
}

impl<S: Clone, U: Update<S>, Rx: Receiver<U>> Follower<S, U, Rx> {
    pub fn new(state: S, leader: Rx) -> Self {
        Self {
            state,
            leader,
            _update: std::marker::PhantomData,
        }
    }
}

impl<S: Clone, U: Update<S> + Clone, Rx: Receiver<U>> Follower<S, U, Rx> {
    pub fn read(&mut self) -> &S {
        loop {
            match self.leader.try_recv() {
                Ok(update) => {
                    update.apply(&mut self.state);
                }
                Err(_) => return &self.state,
            }
        }
    }
}

impl<S: Clone, U: Update<S> + Clone, Rx: Receiver<Sequenced<U>>>
    Follower<Sequenced<S>, Sequenced<U>, Rx>
{
    /// Returns the sequence number of the state.
    pub fn seq(&self) -> u64 {
        self.state.seq
    }

    /// Convenience function for reading sequenced state without the sequence number.
    pub fn sequenced_read(&mut self) -> &S {
        &self.read().value
    }

    /// Blocks until the sequence number of the state is equal to or greater than the given sequence number, then
    /// returns the value.
    pub fn sequenced_read_at(&mut self, seq: u64) -> Result<&S, Rx::RecvError> {
        while self.state.seq < seq {
            self.leader.recv()?.apply(&mut self.state);
        }
        Ok(&self.state.value)
    }
}

// The most convenient way to construct a pair
pub trait IntoLeaderFollower<S: Clone, U: Update<S>, Tx: Sender<U>, Rx: Receiver<U>> {
    fn into_leader_follower(self, state: S) -> (Leader<S, U, Tx>, Follower<S, U, Rx>);
}

impl<S: Clone, U: Update<S>, Tx: Sender<U>, Rx: Receiver<U>> IntoLeaderFollower<S, U, Tx, Rx>
    for (Tx, Rx)
{
    fn into_leader_follower(self, state: S) -> (Leader<S, U, Tx>, Follower<S, U, Rx>) {
        let (tx, rx) = self;
        let leader = Leader::new(state.clone(), tx);
        let follower = Follower::new(state, rx);

        (leader, follower)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let (mut leader, mut follower) = mpsc::channel().into_leader_follower(420);

        assert_eq!(*follower.read(), 420);

        leader.update(69).unwrap();

        assert_eq!(*follower.read(), 69);
    }

    #[test]
    fn test_seq() {
        let (mut leader, mut follower) = mpsc::channel().into_leader_follower(Sequenced::new(420));

        assert_eq!(leader.seq(), 0);

        leader.sequenced_update(69).unwrap();

        assert_eq!(leader.seq(), 1);
        assert_eq!(follower.seq(), 0);

        assert_eq!(*follower.sequenced_read(), 69);

        assert_eq!(follower.seq(), 1);
    }

    #[test]
    fn test_seq_thread() {
        let (mut leader, mut follower) = mpsc::channel().into_leader_follower(Sequenced::new(420));

        assert_eq!(leader.seq(), 0);

        let join_handle = std::thread::spawn(move || *follower.sequenced_read_at(1).unwrap());

        leader.sequenced_update(69).unwrap();

        assert_eq!(join_handle.join().unwrap(), 69);
    }
}
