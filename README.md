`second-stack-vec` serves a similar purpose to [`second-stack`](https://github.com/That3Percent/second-stack) but has a
simpler implementation (it doesn't use any segmentation) at the cost a less ergonomic API. Essentially it requires a
mutable reference be threaded though-out the program. Each of the types a constant parameter `A` that controls the
maximum supported alignment. It defaults to 16 since it seems to be the maximum alignment required without using 
`#[align(...)]`.

# Warning
This library uses `unsafe` Rust, but has not been thoroughly audited

# Example
```rust
use second_stack_vec::*;
struct Defer<F: FnMut()>(F);
impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        self.0()
    }
}

let mut stack = StackMemory::<8>::new();
stack.stack().with_vec(|mut s| {
    s.push(1u8);
    s.push(2);
    s.push(3);
    let mut next = 0;
    s.stack().with_vec(|mut s| {
        s.push((Defer(|| next = 4), 42u16));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, 42);
    });
    s.push(next);
    assert_eq!(&*s, &[1, 2, 3, 4]);
})
```