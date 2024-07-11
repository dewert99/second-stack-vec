use core::fmt::{Debug, Formatter};
use core::ops::Deref;

use crate::{StackMemory, StackVec};

impl<'a, T: Debug, const A: usize> Debug for StackVec<'a, T, A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.deref().fmt(f)
    }
}

impl<'a, T, const A: usize> Extend<T> for StackVec<'a, T, A> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        iter.into_iter().for_each(|e| self.push(e))
    }
}

impl<const A: usize> Default for StackMemory<A> {
    fn default() -> Self {
        StackMemory::new()
    }
}
