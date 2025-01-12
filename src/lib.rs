use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

/// A trait for **deterministic** updates. By using this trait, you herby swear the following solemn oath:
/// > I swear by my life and my love of it that I will never implement this trait in a way that is not deterministic.
pub trait Update<T>: Sized {
    fn apply(&self, target: &mut T);
    fn apply_final(self, target: &mut T) {
        self.apply(target);
    }
}

impl<F, T> Update<T> for F
where
    F: Fn(&mut T),
{
    fn apply(&self, target: &mut T) {
        self(target);
    }
}

/// A double buffer for state machines.
pub struct DoubleBuffer<State> {
    state_a: Arc<RwLock<State>>,
    state_b: Arc<RwLock<State>>,
    read_a: Arc<AtomicBool>,
}

impl<State> DoubleBuffer<State> {
    pub fn new(initial_state: State, initial_state_clone: State) -> Self {
        let state_a = Arc::new(RwLock::new(initial_state));
        let state_b = Arc::new(RwLock::new(initial_state_clone));
        let read_a = Arc::new(AtomicBool::new(false));
        Self {
            state_a,
            state_b,
            read_a,
        }
    }

    pub fn read(&self) -> RwLockReadGuard<State> {
        if self.read_a.load(Ordering::Relaxed) {
            self.state_a.read().unwrap()
        } else {
            self.state_b.read().unwrap()
        }
    }

    fn write(&self) -> RwLockWriteGuard<State> {
        if self.read_a.load(Ordering::Relaxed) {
            self.state_b.write().unwrap()
        } else {
            self.state_a.write().unwrap()
        }
    }

    fn swap(&self) {
        self.read_a.fetch_not(Ordering::Relaxed);
    }

    pub fn apply<U: Update<State>>(&self, update: U) {
        update.apply(&mut self.write());
        self.swap();
        update.apply(&mut self.write());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i32() {
        let buffer = DoubleBuffer::new(0, 0);
        buffer.apply(|x: &mut i32| *x += 1);
        assert_eq!(*buffer.read(), 1);
    }
}
