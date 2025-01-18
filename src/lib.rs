use std::{ops::Deref, sync::mpsc};

/// A trait for **deterministic** updates. By using this trait, you herby swear the following solemn oath:
/// > I swear by my life and my love of it that I will never implement this trait in a way that is not deterministic.
pub trait Update<T>: Sized {
    fn apply(&self, target: &mut T);
}

impl<T> Update<T> for T
where
    T: Copy,
{
    fn apply(&self, target: &mut T) {
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
        for subscriber in &self.subscribers {
            subscriber.send(update.clone()).unwrap();
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
}

impl<S: Clone, U: Update<S> + Clone> Follower<S, U> {
    pub fn new(state: S, subscription: mpsc::Receiver<U>) -> Self {
        Self {
            state,
            subscription,
        }
    }

    pub fn update(&mut self) {
        while let Ok(update) = self.subscription.try_recv() {
            update.apply(&mut self.state);
        }
    }

    pub fn get(&self) -> Guard<S> {
        Guard { state: &self.state }
    }
}

pub struct Guard<'a, S> {
    state: &'a S,
}

impl<S> Deref for Guard<'_, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.state
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let leader = &mut Leader::new(420);
        let follower = &mut leader.follow();

        leader.update(69);

        assert!(*follower.get().state == 420);

        follower.update();

        assert!(*follower.get().state == 69);
    }
}
