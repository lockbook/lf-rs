use std::{
    ops::Deref,
    sync::mpsc::{self, RecvError, SendError, TryRecvError},
};

/// A trait for **deterministic** updates. By using this trait, you herby swear the following solemn oath:
/// > I swear by my life and my love of it that I will never implement this trait in a way that is not deterministic.
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

#[derive(Debug)]
pub struct Leader<S, U: Update<S>> {
    state: S,
    subscribers: Vec<mpsc::Sender<U>>,
}

impl<S, U: Update<S>> Deref for Leader<S, U> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S: Clone, U: Update<S> + Clone> Leader<S, U> {
    pub fn new(state: S) -> Self {
        Self {
            state,
            subscribers: Vec::new(),
        }
    }

    pub fn update(&mut self, update: U)
    where
        U: Update<S>,
    {
        update.apply(&mut self.state);

        for i in (0..self.subscribers.len()).rev() {
            if let Err(SendError(_)) = self.subscribers[i].send(update.clone()) {
                self.subscribers.remove(i);
            }
        }
    }

    pub fn follow(&mut self) -> Follower<S, U> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        Follower::new(self.state.clone(), rx)
    }
}

#[derive(Debug)]
pub struct Follower<S, U: Update<S>> {
    state: S,
    subscription: mpsc::Receiver<U>,
    subscribers: Vec<mpsc::Sender<U>>,
}

impl<S: Clone, U: Update<S> + Clone> Follower<S, U> {
    pub fn new(state: S, subscription: mpsc::Receiver<U>) -> Self {
        Self {
            state,
            subscription,
            subscribers: Vec::new(),
        }
    }

    /// Applies all available updates.
    pub fn try_receive_updates(&mut self) -> TryRecvError {
        loop {
            match self.subscription.try_recv() {
                Ok(update) => {
                    update.apply(&mut self.state);
                    for i in (0..self.subscribers.len()).rev() {
                        if let Err(SendError(_)) = self.subscribers[i].send(update.clone()) {
                            self.subscribers.remove(i);
                        }
                    }
                }
                Err(e) => return e,
            }
        }
    }

    /// Applies all available updates, waiting for at least one update to become available.
    pub fn receive_updates(&mut self) -> Result<(), RecvError> {
        let update = self.subscription.recv()?;
        update.apply(&mut self.state);
        for i in (0..self.subscribers.len()).rev() {
            if let Err(SendError(_)) = self.subscribers[i].send(update.clone()) {
                self.subscribers.remove(i);
            }
        }

        match self.try_receive_updates() {
            TryRecvError::Disconnected => Err(RecvError),
            TryRecvError::Empty => Ok(()),
        }
    }

    pub fn follow(&mut self) -> Follower<S, U> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        Follower::new(self.state.clone(), rx)
    }
}

impl<S, U: Update<S>> Deref for Follower<S, U> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let mut leader = Leader::new(420);
        let mut follower = leader.follow();

        leader.update(69);

        let f: i32 = *follower;
        assert_eq!(f, 420);

        follower.try_receive_updates();

        let f: i32 = *follower;
        assert_eq!(f, 69);
    }

    #[test]
    fn test_relay() {
        let mut leader = Leader::new(420);
        let mut follower = leader.follow();
        let mut follower2 = follower.follow();

        leader.update(69);

        let f: i32 = *follower;
        assert_eq!(f, 420);

        follower.try_receive_updates();

        let f: i32 = *follower;
        assert_eq!(f, 69);

        let f: i32 = *follower2;
        assert_eq!(f, 420);

        follower2.try_receive_updates();

        let f: i32 = *follower2;
        assert_eq!(f, 69);
    }
}
