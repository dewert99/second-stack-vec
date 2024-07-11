use aligned_vec::{AVec, ConstAlign};
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};

/// Base type used for working with [`StackVec`].
/// It is generally constructed in a top level / long-running function where it will use
/// [`StackMemory::stack`] to create a [`Stack`] which can be passed to helper functions that
/// want to make use of the second stack memory
pub struct StackMemory<const A: usize = 16>(AVec<MaybeUninit<u8>, ConstAlign<A>>);

impl<const A: usize> StackMemory<A> {
    /// Create a secondary stack
    pub fn new() -> Self {
        StackMemory(AVec::new(0))
    }

    /// Returns a handle used for working with this stack
    pub fn stack(&mut self) -> Stack<'_, A> {
        Stack(self)
    }
}

/// Untyped handle for creating [`StackVec`]s.
/// It is generally used as the argument for functions that want use of the second stack memory.
/// It is initially created with [`StackMemory::stack`], but can also be created from
/// [`StackVec::stack`] to reuse an existing stack.
// Invariant: self.stack.0 will have the same length and contents don't change between calls to any
// methods
pub struct Stack<'a, const A: usize = 16>(&'a mut StackMemory<A>);

impl<'a, const A: usize> Stack<'a, A> {
    /// Allocates a [`StackVec`] on this stack and passes it to `f`
    ///
    /// ## Requires (Compile time const assert):
    /// `A % mem::align_of<T>() == 0 && mem::size_of::<T> > 0`
    pub fn with_vec<T, U>(&mut self, f: impl FnOnce(StackVec<'_, T, A>) -> U) -> U {
        const {
            assert!(
                A % mem::align_of::<T>() == 0,
                "The stacks alignment is not a multiple of the types alignment"
            )
        };
        const { assert!(mem::size_of::<T>() > 0, "ZSTs are not supported") };
        let end_ptr = self.0 .0.as_mut_ptr_range().end;
        let offset = end_ptr.align_offset(mem::align_of::<T>());
        let old_len = self.0 .0.len();
        let new_len = old_len + offset;
        self.0 .0.resize(new_len, MaybeUninit::uninit());
        debug_assert_eq!(self.0 .0.len() % mem::align_of::<T>(), 0);
        // Invariant Creation: self.0.0[new_len..] is a valid [T] since it is empty
        // Invariant Creation: self.0.0.len() % mem::align_of::<T>() == 0 because we added the
        // appropriate offset
        let res = f(StackVec {
            base: new_len,
            stack: &mut *self.0,
            phantom: PhantomData,
        });
        // Safety: self.0.0[self.base..] is a valid [T] from the StackVec invariant
        let ptr = unsafe { self.0 .0.as_mut_ptr().add(new_len) } as *mut MaybeUninit<T>;
        let added_len = (self.0 .0.len() - new_len) / mem::size_of::<T>();
        let added_elts = unsafe { core::slice::from_raw_parts_mut(ptr, added_len) };
        for elt in added_elts {
            unsafe { elt.assume_init_drop() }
        }
        // Invariant Preservation: self.0.0[..old_len] couldn't have been changed by f because of
        // the StackVec invariant, and we now truncate to old_len ensuring we have the same length
        // and contents
        self.0 .0.truncate(old_len);
        res
    }
}

// Invariant: A % mem::align_of::<T>() == 0
// Invariant: self.stack.0.len() % mem::align_of::<T>() == 0
// Invariant: mem::size_of::<T>() > 0
// Invariant: self.stack.0[self.base..] is a valid [T]
// Invariant: self.stack.0[..self.base] never changes
/// A variant of [`Vec`](alloc::vec::Vec) that stores elements inline on a second-stack
/// A [`StackVec`] can be mutably borrowed to create a [`Stack`] which can create more [`StackVec`]s
/// which reuse the same allocation using [`StackVec::stack`].
pub struct StackVec<'a, T, const A: usize = 16> {
    base: usize,
    stack: &'a mut StackMemory<A>,
    phantom: PhantomData<T>,
}

impl<'a, T, const A: usize> StackVec<'a, T, A> {
    /// Appends an element to the back of a collection.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` _bytes_.
    ///
    /// # Examples
    ///
    /// ```
    /// use second_stack_vec::StackMemory;
    /// StackMemory::<4>::new().stack().with_vec(|mut vec| {
    ///     vec.push(1);
    ///     vec.push(2);
    ///     vec.push(3);
    ///     assert_eq!(&*vec, &[1, 2, 3]);
    /// })
    /// ```
    ///
    /// # Time complexity
    ///
    /// Takes amortized *O*(1) time. If the vector's length would exceed its
    /// capacity after the push, *O*(*capacity*) time is taken to copy the
    /// vector's elements to a larger allocation. This expensive operation is
    /// offset by the *capacity* *O*(1) insertions it allows.
    pub fn push(&mut self, val: T) {
        let old_len = self.stack.0.len();
        // Invariant Preservation:
        // Since old_len % mem::align_of::<T>() == 0
        // and mem::size_of::<T>() % mem::align_of::<T>() == 0,
        // old_len + mem::size_of::<T>() % mem::align_of::<T>() == 0
        self.stack
            .0
            .resize(old_len + mem::size_of::<T>(), MaybeUninit::uninit());
        let ptr = unsafe { self.stack.0.as_mut_ptr().add(old_len) } as *mut T;
        // Safety Alignment:
        // Since self.stack.as_ptr() is aligned for A (since it's an AVec)
        // and A % mem::align_of::<T>() == 0, self.stack.as_ptr() is also aligned for T
        // Since old_len % mem::align_of::<T>() == 0, self.stack.0[old_len].as_ptr() is aligned for T
        // Safety Validity: self.stack.0.len() == old_len + mem::size_of::<T>(),
        // so there are mem::size_of::<T>() bytes available past the end of old_len
        // Invariant Preservation:
        // self.stack.0[self.base..] is still a valid [T] since we added one extra T to the end
        unsafe { ptr.write(val) }
    }

    /// mutably borrowed `self` to create a [`Stack`] which can create more [`StackVec`]s
    /// which reuse the same allocation using [`StackVec::stack`].
    /// After the returned [`Stack`] goes out of scope the length and contents of `self` will be
    /// left unchanged
    pub fn stack(&mut self) -> Stack<'_, A> {
        Stack(&mut *self.stack)
    }
}

impl<'a, T, const A: usize> Deref for StackVec<'a, T, A> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        // Safety:  self.stack.0[self.base..] is a valid [T] from invariant
        let ptr = unsafe { self.stack.0.as_ptr().add(self.base) } as *const T;
        let len = (self.stack.0.len() - self.base) / mem::size_of::<T>();
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }
}

impl<'a, T, const A: usize> DerefMut for StackVec<'a, T, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety:  self.stack.0[self.base..] is a valid [T] from invariant
        let ptr = unsafe { self.stack.0.as_mut_ptr().add(self.base) } as *mut T;
        let len = (self.stack.0.len() - self.base) / mem::size_of::<T>();
        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }
}
