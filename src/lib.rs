use std::{
    ops::Deref,
    sync::mpsc::{self, RecvError, SendError, TryRecvError},
};

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

// Leader
#[derive(Debug)]
pub struct Leader<S, U: Update<S>> {
    state: State<S>,
    subscribers: Vec<mpsc::Sender<(U, u64)>>,
}

impl<S, U: Update<S>> Deref for Leader<S, U> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state.snapshot
    }
}

impl<S: Clone, U: Update<S> + Clone> Leader<S, U> {
    pub fn new(state: S) -> Self {
        Self {
            state: State {
                snapshot: state,
                seq: 0,
            },
            subscribers: Vec::new(),
        }
    }

    pub fn update(&mut self, update: U) -> u64
    where
        U: Update<S>,
    {
        update.apply(&mut self.state.snapshot);
        self.state.seq += 1;

        for i in (0..self.subscribers.len()).rev() {
            if let Err(SendError(_)) = self.subscribers[i].send((update.clone(), self.state.seq)) {
                self.subscribers.remove(i);
            }
        }

        self.state.seq
    }

    pub fn follow(&mut self) -> Follower<S, U> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        Follower::new(self.state.clone(), rx)
    }

    pub fn seq(&self) -> u64 {
        self.state.seq
    }
}

// Follower
#[derive(Debug)]
pub struct Follower<S, U: Update<S>> {
    state: State<S>,
    subscription: mpsc::Receiver<(U, u64)>,
    subscribers: Vec<mpsc::Sender<(U, u64)>>,
    waiters: Vec<(pulse::Pulse, u64)>,
}

impl<S: Clone, U: Update<S> + Clone> Follower<S, U> {
    pub fn new(state: State<S>, subscription: mpsc::Receiver<(U, u64)>) -> Self {
        Self {
            state,
            subscription,
            subscribers: Vec::new(),
            waiters: Vec::new(),
        }
    }

    /// Applies all available updates.
    pub fn refresh(&mut self) -> TryRecvError {
        loop {
            match self.subscription.try_recv() {
                Ok((update, seq)) => {
                    self.apply(update, seq);
                }
                Err(e) => return e,
            }
        }
    }

    /// Applies all available updates, waiting for at least one update to become available.
    pub fn refresh_blocking(&mut self) -> Result<(), RecvError> {
        let (update, seq) = self.subscription.recv()?;
        self.apply(update, seq);
        match self.refresh() {
            TryRecvError::Disconnected => Err(RecvError),
            TryRecvError::Empty => Ok(()),
        }
    }

    pub fn follow(&mut self) -> Follower<S, U> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        Follower::new(self.state.clone(), rx)
    }

    pub fn seq(&self) -> u64 {
        self.state.seq
    }

    /// Returns a pulse::Signal which can be used to wait for the follower to reach a certain sequence number in sync
    /// or async contexts.
    pub fn signal_at(&mut self, seq: u64) -> pulse::Signal {
        let (signal, pulse) = pulse::Signal::new();
        self.waiters.push((pulse, seq));
        self.check_waiters();
        signal
    }

    fn apply(&mut self, update: U, seq: u64) {
        update.apply(&mut self.state.snapshot);
        self.state.seq = seq;

        for i in (0..self.subscribers.len()).rev() {
            if let Err(SendError(_)) = self.subscribers[i].send((update.clone(), self.state.seq)) {
                self.subscribers.remove(i);
            }
        }

        self.check_waiters();
    }

    fn check_waiters(&mut self) {
        for i in (0..self.waiters.len()).rev() {
            let (_, seq) = &self.waiters[i];
            if *seq <= self.state.seq {
                let (pulse, _) = self.waiters.remove(i);
                pulse.pulse();
            }
        }
    }
}

impl<S, U: Update<S>> Deref for Follower<S, U> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state.snapshot
    }
}

#[cfg(test)]
mod test {
    use std::thread;

    use super::*;

    #[test]
    fn test() {
        let mut leader = Leader::new(420);
        let mut follower = leader.follow();

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
        let mut follower = leader.follow();

        assert_eq!(leader.seq(), 0);

        leader.update(69);

        assert_eq!(leader.seq(), 1);
        assert_eq!(follower.seq(), 0);

        follower.refresh();

        assert_eq!(follower.seq(), 1);
    }

    // change notification: update relay
    #[test]
    fn test_relay() {
        let mut leader = Leader::new(420);
        let mut follower = leader.follow();
        let mut follower2 = follower.follow();

        leader.update(69);

        let f: i32 = *follower;
        assert_eq!(f, 420);

        follower.refresh();

        let f: i32 = *follower;
        assert_eq!(f, 69);

        let f: i32 = *follower2;
        assert_eq!(f, 420);

        follower2.refresh();

        let f: i32 = *follower2;
        assert_eq!(f, 69);
    }

    // change notification: blocking refresh
    #[test]
    fn test_blocking() {
        let mut leader = Leader::new(420);
        let mut follower = leader.follow();

        let handle = thread::spawn(move || {
            leader.update(69);
        });

        let f: i32 = *follower;
        assert_eq!(f, 420);

        let _ = follower.refresh_blocking();

        let f: i32 = *follower;
        assert_eq!(f, 69);

        handle.join().unwrap();
    }

    // change notification: pulse
    #[test]
    fn test_pulse() {
        let mut leader = Leader::new(420);
        let mut follower = leader.follow();

        let seq = leader.update(69);
        let signal = follower.signal_at(seq);

        let f: i32 = *follower;
        assert_eq!(f, 420);

        let _ = follower.refresh_blocking();

        let f: i32 = *follower;
        assert_eq!(f, 69);

        signal.wait().unwrap();
    }
}
