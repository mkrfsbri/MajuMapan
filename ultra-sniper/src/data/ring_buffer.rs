use crate::data::candle::Candle;

/// Fixed-capacity ring buffer backed by a const-generic array.
/// O(1) insert, zero heap allocation.
pub struct RingBuffer<T: Copy, const N: usize> {
    buf:  [Option<T>; N],
    head: usize,
    len:  usize,
}

impl<T: Copy, const N: usize> RingBuffer<T, N> {
    pub const fn new() -> Self {
        Self { buf: [None; N], head: 0, len: 0 }
    }

    /// Push a value, overwriting oldest when full.
    #[inline]
    pub fn push(&mut self, value: T) {
        self.buf[self.head] = Some(value);
        self.head = (self.head + 1) % N;
        if self.len < N { self.len += 1; }
    }

    pub fn len(&self)      -> usize { self.len }
    pub fn is_empty(&self) -> bool  { self.len == 0 }
    pub fn is_full(&self)  -> bool  { self.len == N }

    /// Most recently pushed element.
    pub fn latest(&self) -> Option<T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + N - 1) % N]
    }

    /// Logical index: 0 = oldest, len-1 = newest.
    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.len { return None; }
        let oldest = if self.len < N { 0 } else { self.head };
        self.buf[(oldest + i) % N]
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.len).filter_map(|i| self.get(i))
    }
}

// ── candle-specific alias used by the aggregator ──────────────────────────────
pub type CandleBuffer<const N: usize> = RingBuffer<Candle, N>;

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_len() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 3);
        assert!(rb.is_full());
    }

    #[test]
    fn overwrite_oldest() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(1); rb.push(2); rb.push(3); rb.push(4);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(1), Some(3));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn latest_value() {
        let mut rb: RingBuffer<i32, 5> = RingBuffer::new();
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.latest(), Some(30));
    }

    #[test]
    fn empty_latest_is_none() {
        let rb: RingBuffer<i32, 4> = RingBuffer::new();
        assert_eq!(rb.latest(), Option::<i32>::None);
    }

    #[test]
    fn wrap_around_correctness() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        for i in 1..=6 { rb.push(i); }
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
        assert_eq!(rb.get(2), Some(6));
    }

    #[test]
    fn iter_oldest_to_newest() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(7); rb.push(8); rb.push(9); rb.push(10);
        let v: Vec<_> = rb.iter().collect();
        assert_eq!(v, vec![8, 9, 10]);
    }
}
