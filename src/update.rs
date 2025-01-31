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
