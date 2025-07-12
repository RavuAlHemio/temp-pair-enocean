#![cfg_attr(not(test), no_std)]


use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::mem::MaybeUninit;


pub struct RingBuffer<T, const SIZE: usize> {
    buffer: [MaybeUninit<T>; SIZE],
    read_pos: usize,
    write_pos: usize,
}
impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    pub const fn new() -> Self {
        let buffer = [const { MaybeUninit::uninit() }; SIZE];
        Self {
            buffer,
            read_pos: 0,
            write_pos: 0,
        }
    }

    pub const fn len(&self) -> usize {
        let mut len_pos = self.read_pos;
        let mut length = 0;
        while len_pos != self.write_pos {
            len_pos = (len_pos + 1) % SIZE;
            length += 1;
        }
        length
    }

    pub const fn iter(&self) -> Iter<'_, T, SIZE> {
        Iter {
            ring_buffer: self,
            iter_pos: self.read_pos,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.write_pos == self.read_pos
    }

    pub const fn is_full(&self) -> bool {
        (self.write_pos + 1) % SIZE == self.read_pos
    }

    pub fn write(&mut self, value: T) -> bool {
        if self.is_full() {
            return false;
        }

        self.buffer[self.write_pos] = MaybeUninit::new(value);
        self.write_pos = (self.write_pos + 1) % SIZE;
        true
    }

    pub fn peek(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }

        let reference = unsafe {
            self.buffer[self.read_pos].assume_init_ref()
        };
        Some(reference)
    }

    pub fn read(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let value = unsafe {
            self.buffer[self.read_pos].assume_init_read()
        };
        self.buffer[self.read_pos] = MaybeUninit::uninit();
        self.read_pos = (self.read_pos + 1) % SIZE;
        Some(value)
    }
}
impl<T: Clone, const SIZE: usize> Clone for RingBuffer<T, SIZE> {
    fn clone(&self) -> Self {
        let mut cloned = Self::new();

        // copy only those elements that we know are initialized
        let mut my_read_pos = self.read_pos;
        for item in self.iter() {
            cloned.buffer[my_read_pos] = MaybeUninit::new(item.clone());
            my_read_pos = (my_read_pos + 1) % SIZE;
        }

        cloned.read_pos = self.read_pos;
        cloned.write_pos = self.write_pos;

        cloned
    }
}
impl<T: fmt::Debug, const SIZE: usize> fmt::Debug for RingBuffer<T, SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RingBuffer([")?;
        let mut first_item = true;
        for item in self.iter() {
            if first_item {
                first_item = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", item)?;
        }
        write!(f, "])")?;
        Ok(())
    }
}
impl<T, const SIZE: usize> Default for RingBuffer<T, SIZE> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T, const SIZE: usize> Drop for RingBuffer<T, SIZE> {
    fn drop(&mut self) {
        // drop those elements that we know are initialized
        let mut drop_pos = self.read_pos;
        while drop_pos != self.write_pos {
            unsafe { self.buffer[drop_pos].assume_init_drop() };
            drop_pos = (drop_pos + 1) % SIZE;
        }
    }
}
impl<T: PartialEq, const SIZE: usize> PartialEq for RingBuffer<T, SIZE> {
    fn eq(&self, other: &Self) -> bool {
        let mut self_iter = self.iter();
        let mut other_iter = other.iter();
        loop {
            let self_item = self_iter.next();
            let other_item = other_iter.next();
            match (self_item, other_item) {
                (Some(s), Some(o)) => {
                    if s != o {
                        return false;
                    }
                    // continue otherwise
                },
                (None, None) => {
                    return true;
                },
                _ => {
                    // buffers are of different lengths
                    return false;
                },
            }
        }
    }
}
impl<T: Eq, const SIZE: usize> Eq for RingBuffer<T, SIZE> {
}
impl<T: Hash, const SIZE: usize> Hash for RingBuffer<T, SIZE> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut iter = self.iter();
        while let Some(item) = iter.next() {
            item.hash(state);
        }
    }
}
impl<T: PartialOrd, const SIZE: usize> PartialOrd for RingBuffer<T, SIZE> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_iter = self.iter();
        let mut other_iter = other.iter();
        loop {
            let self_item = self_iter.next();
            let other_item = other_iter.next();
            match (self_item, other_item) {
                (Some(s), Some(o)) => {
                    match s.partial_cmp(o) {
                        Some(Ordering::Equal) => {}, // keep going
                        other => return other, // less/greater/unknown, it's decided
                    }
                },
                (None, None) => {
                    // we got this far, they're equal
                    return Some(Ordering::Equal);
                },
                (Some(_), None) => {
                    // self is longer than other
                    return Some(Ordering::Greater);
                },
                (None, Some(_)) => {
                    // other is longer than self
                    return Some(Ordering::Less);
                },
            }
        }
    }
}
impl<T: Ord, const SIZE: usize> Ord for RingBuffer<T, SIZE> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

pub struct Iter<'a, T, const SIZE: usize> {
    ring_buffer: &'a RingBuffer<T, SIZE>,
    iter_pos: usize,
}
impl<'a, T, const SIZE: usize> Iterator for Iter<'a, T, SIZE> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iter_pos == self.ring_buffer.write_pos {
            return None;
        }

        let value = unsafe {
            self.ring_buffer.buffer[self.iter_pos].assume_init_ref()
        };
        self.iter_pos = (self.iter_pos + 1) % SIZE;
        Some(value)
    }
}


#[cfg(test)]
mod tests {
    use super::RingBuffer;

    fn new_buffer() -> RingBuffer<u8, 4> { RingBuffer::new() }

    #[test]
    pub fn test_empty() {
        let mut buf = new_buffer();
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert!(!buf.is_full());

        let mut iter = buf.iter();
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);

        assert_eq!(buf.peek(), None);
        assert_eq!(buf.read(), None);
    }

    #[test]
    pub fn test_read_write() {
        let mut buf = new_buffer();
        assert_eq!(buf.write(3), true);

        assert_eq!(buf.len(), 1);
        assert!(!buf.is_empty());
        assert!(!buf.is_full());

        let mut iter = buf.iter();
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(buf.len(), 1);

        assert_eq!(buf.peek(), Some(&3));
        assert_eq!(buf.len(), 1);

        assert_eq!(buf.read(), Some(3));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    pub fn test_fill() {
        let mut buf = new_buffer();
        assert_eq!(buf.write(3), true);
        assert_eq!(buf.write(4), true);
        assert_eq!(buf.write(5), true);
        assert_eq!(buf.write(6), false);

        assert_eq!(buf.len(), 3);
        assert!(!buf.is_empty());
        assert!(buf.is_full());

        let mut iter = buf.iter();
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), Some(&4));
        assert_eq!(iter.next(), Some(&5));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(buf.len(), 3);

        assert_eq!(buf.peek(), Some(&3));
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.read(), Some(3));
        assert_eq!(buf.len(), 2);

        assert_eq!(buf.peek(), Some(&4));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.read(), Some(4));
        assert_eq!(buf.len(), 1);

        assert_eq!(buf.peek(), Some(&5));
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.read(), Some(5));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    pub fn test_wrwr() {
        let mut buf = new_buffer();
        assert_eq!(buf.write(3), true);
        assert_eq!(buf.write(4), true);
        assert_eq!(buf.write(5), true);

        assert_eq!(buf.len(), 3);
        assert!(!buf.is_empty());
        assert!(buf.is_full());

        {
            let mut iter = buf.iter();
            assert_eq!(iter.next(), Some(&3));
            assert_eq!(iter.next(), Some(&4));
            assert_eq!(iter.next(), Some(&5));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(buf.len(), 3);
        }

        assert_eq!(buf.read(), Some(3));
        assert_eq!(buf.len(), 2);

        assert_eq!(buf.write(6), true);

        {
            let mut iter = buf.iter();
            assert_eq!(iter.next(), Some(&4));
            assert_eq!(iter.next(), Some(&5));
            assert_eq!(iter.next(), Some(&6));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(buf.len(), 3);
        }

        assert_eq!(buf.peek(), Some(&4));
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.read(), Some(4));
        assert_eq!(buf.len(), 2);

        assert_eq!(buf.peek(), Some(&5));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.read(), Some(5));
        assert_eq!(buf.len(), 1);

        assert_eq!(buf.peek(), Some(&6));
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.read(), Some(6));
        assert_eq!(buf.len(), 0);
    }
}
