/// Fixed-capacity ring buffer backed by a const-generic array.
/// O(1) insert, zero heap allocation.
pub struct RingBuffer<T: Copy, const N: usize> {
    buf:   [Option<T>; N],
    head:  usize,  // next write position
    len:   usize,  // current number of valid elements (≤ N)
}

impl<T: Copy, const N: usize> RingBuffer<T, N> {
    pub const fn new() -> Self {
        Self {
            buf:  [None; N],
            head: 0,
            len:  0,
        }
    }

    /// Push an element. Overwrites the oldest entry when full.
    #[inline]
    pub fn push(&mut self, value: T) {
        self.buf[self.head] = Some(value);
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    /// Number of valid elements currently stored.
    #[inline]
    pub fn len(&self) -> usize { self.len }

    /// True when no elements have been stored yet.
    #[inline]
    pub fn is_empty(&self) -> bool { self.len == 0 }

    /// True when the buffer holds exactly N elements.
    #[inline]
    pub fn is_full(&self) -> bool { self.len == N }

    /// Access the most recently pushed element.
    pub fn latest(&self) -> Option<T> {
        if self.len == 0 { return None; }
        let idx = (self.head + N - 1) % N;
        self.buf[idx]
    }

    /// Access by logical index 0 = oldest, len-1 = newest.
    pub fn get(&self, logical_idx: usize) -> Option<T> {
        if logical_idx >= self.len { return None; }
        // Oldest slot:
        let oldest = if self.len < N {
            0
        } else {
            self.head          // head is past the oldest when full
        };
        let physical = (oldest + logical_idx) % N;
        self.buf[physical]
    }

    /// Iterate from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.len).filter_map(move |i| self.get(i))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Step 2 Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_within_capacity() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.len(), 3);
        assert!(rb.is_full());
    }

    #[test]
    fn fourth_push_overwrites_oldest() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4); // 1 is overwritten
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2)); // oldest is now 2
        assert_eq!(rb.get(1), Some(3));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn latest_returns_most_recent() {
        let mut rb: RingBuffer<i32, 5> = RingBuffer::new();
        rb.push(10);
        rb.push(20);
        rb.push(30);
        assert_eq!(rb.latest(), Some(30));
    }

    #[test]
    fn latest_on_empty_buffer_is_none() {
        let rb: RingBuffer<i32, 4> = RingBuffer::new();
        assert_eq!(rb.latest(), Option::<i32>::None);
    }

    #[test]
    fn wrap_around_index_correctness() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        for i in 1..=6 {
            rb.push(i);
        }
        // After pushing 1..6 with size 3, buffer holds [4,5,6]
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
        assert_eq!(rb.get(2), Some(6));
    }

    #[test]
    fn iter_order_oldest_to_newest() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(7);
        rb.push(8);
        rb.push(9);
        rb.push(10); // overwrites 7
        let collected: Vec<i32> = rb.iter().collect();
        assert_eq!(collected, vec![8, 9, 10]);
    }
}
