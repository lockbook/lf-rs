mod map;
mod sequenced;
mod transport;
mod update;

use map::SendMap;
// pub use map::Map;
pub use sequenced::Sequenced;
pub use transport::{Receiver, RecvError, SendError, Sender, TryRecvError};
pub use update::Update;

/// Leaders are state replicas that both apply and emit updates submitted by callers. Followers follow leaders,
/// replicating their state by applying the emitted updates.
#[derive(Debug)]
pub struct Leader<S, U: Update<S>, Tx: Sender<Message = U>> {
    state: S,
    follower: Option<Tx>,
    _update: std::marker::PhantomData<U>,
}

impl<S: Clone, U: Update<S>, Tx: Sender<Message = U>> Leader<S, U, Tx> {
    /// Create a leader with the given state and follower.
    pub fn new(state: S) -> Self {
        Self {
            state,
            follower: None,
            _update: std::marker::PhantomData,
        }
    }

    /// Apply the update to this leader and send it to the follower.
    pub fn update(&mut self, update: U) -> Result<(), SendError<U, Tx::Error>>
    where
        U: Update<S>,
    {
        update.apply(&mut self.state);

        self.follower
            .as_ref()
            .map(|f| f.send(update))
            .unwrap_or(Ok(()))
    }

    /// Attaches the given sender to the leader and returns a mutable reference for further operations.
    pub fn lead(&mut self, follower: Tx) -> &mut Tx {
        self.follower = Some(follower);
        self.follower.as_mut().unwrap()
    }
}

trait Lead<U, O, F: Fn(&U) -> O, Tx: Sender<Message = O>> {
    /// Attaches the given sender to the leader and returns a mutable reference for further operations.
    fn lead(&mut self, follower: SendMap<U, O, F, Tx>) -> &mut SendMap<U, O, F, Tx>;

    fn map(&mut self, f: F) -> &mut SendMap<U, O, F, Tx> {
        self.lead(SendMap::new(f))
    }
}

impl<S: Clone, U: Update<S>, O, F: Fn(&U) -> O, Tx: Sender<Message = O>> Lead<U, O, F, Tx>
    for Leader<S, U, SendMap<U, O, F, Tx>>
{
    fn lead(&mut self, follower: SendMap<U, O, F, Tx>) -> &mut SendMap<U, O, F, Tx> {
        self.follower = Some(follower);
        self.follower.as_mut().unwrap()
    }
}

impl<S: Clone, U: Update<S>, Tx: Sender<Message = Sequenced<U>>>
    Leader<Sequenced<S>, Sequenced<U>, Tx>
{
    /// Get the sequence number of this sequenced leader.
    pub fn seq(&self) -> u64 {
        self.state.seq
    }

    /// Apply the update to this sequenced leader and send it to the follower. Automatically maintains sequence numbers.
    pub fn sequenced_update(&mut self, update: U) -> Result<(), SendError<U, Tx::Error>> {
        self.state.seq += 1;
        let sequenced_update = Sequenced {
            value: update,
            seq: self.state.seq,
        };

        match self.update(sequenced_update) {
            Ok(()) => Ok(()),
            Err(SendError::Disconnected(sequenced_update)) => {
                Err(SendError::Disconnected(sequenced_update.value))
            }
            Err(SendError::Inner(e)) => Err(SendError::Inner(e)),
        }
    }
}

/// Followers are state replicas that only apply updates emitted from the leader. Every observed state of the follower
/// is guaranteed to have been a real state of the leader at some point in time.
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
    /// Read the state of the follower, applying all received updates first.
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

#[cfg(test)]
mod test {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn test() {
        let mut leader = Leader::new(420);
        leader.map(|f| f + 2).map(|f| f + 2);

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
