use crate::Update;

/// Sequenced data has changes that occur at ordered sequence numbers. Transports deliver sequenced data monotonically
/// increasing in order of sequence number.
#[derive(Clone, Debug)]
pub struct Sequenced<S> {
    pub value: S,
    pub seq: u64,
}

impl<S> Sequenced<S> {
    pub fn new(value: S) -> Self {
        Self { value, seq: 0 }
    }
}

// Sequenced updates are updates for sequenced data. The sequence number of the sequenced data will be updated to the
// sequence number of the sequenced update. Such are the updates of the sequence of sequenced updates.
impl<S, U> Update<Sequenced<S>> for Sequenced<U>
where
    U: Update<S>,
{
    fn apply(&self, target: &mut Sequenced<S>) {
        self.value.apply(&mut target.value);
        target.seq = self.seq;
    }
}

#[cfg(test)]
mod test {
    use crate::Update as _;

    use super::Sequenced;

    #[test]
    fn apply() {
        let mut x = Sequenced::new(420);

        assert_eq!(x.seq, 0);

        let y = Sequenced { value: 69, seq: 1 };
        y.apply(&mut x);

        assert_eq!(x.seq, 1);
    }
}
