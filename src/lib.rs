use std::{
    ops::Deref,
    sync::mpsc::{self, SendError, TryRecvError},
};

/// A trait for **deterministic** updates. By using this trait, you herby swear the following solemn oath:
/// > I swear by my life and my love of it that I will never implement this trait in a way that is not deterministic.
pub trait Event<S>: Sized {
    fn apply(&self, target: &mut S);
}

impl<S> Event<S> for S
where
    S: Copy,
{
    fn apply(&self, target: &mut S) {
        *target = *self;
    }
}

#[derive(Debug)]
pub struct Leader<S, E: Event<S>> {
    state: S,
    subscribers: Vec<mpsc::Sender<E>>,
}

impl<S, E: Event<S>> Deref for Leader<S, E> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S: Clone, E: Event<S> + Clone> Leader<S, E> {
    pub fn new(state: S) -> Self {
        Self {
            state,
            subscribers: Vec::new(),
        }
    }

    pub fn update(&mut self, update: E)
    where
        E: Event<S>,
    {
        update.apply(&mut self.state);

        for i in (0..self.subscribers.len()).rev() {
            if let Err(SendError(_)) = self.subscribers[i].send(update.clone()) {
                self.subscribers.remove(i);
            }
        }
    }

    pub fn follow(&mut self) -> Follower<S, E> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        Follower::new(self.state.clone(), rx)
    }
}

#[derive(Debug)]
pub struct Follower<S, E: Event<S>> {
    state: S,
    subscription: mpsc::Receiver<E>,
}

impl<S: Clone, E: Event<S> + Clone> Follower<S, E> {
    pub fn new(state: S, subscription: mpsc::Receiver<E>) -> Self {
        Self {
            state,
            subscription,
        }
    }

    pub fn refresh(&mut self) -> TryRecvError {
        loop {
            match self.subscription.try_recv() {
                Ok(update) => update.apply(&mut self.state),
                Err(e) => break e,
            }
        }
    }
}

impl<S, E: Event<S>> Deref for Follower<S, E> {
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

        let _ = follower.refresh();

        let f: i32 = *follower;
        assert_eq!(f, 69);
    }
}
