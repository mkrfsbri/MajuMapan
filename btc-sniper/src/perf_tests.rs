/// Performance tests — Step 8
///
/// These tests live in a normal `#[cfg(test)]` module and use
/// `std::time::Instant` for wall-clock measurement.  Criterion or other
/// harnesses are not required; the assertions act as regression guards.
#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::types::Candle;
    use crate::pipeline::Pipeline;

    fn make_candle(i: usize) -> Candle {
        let p = 30_000.0 + i as f64 * 0.5;
        Candle::new(p, p + 10.0, p - 10.0, p + 2.0, 1.0)
    }

    /// Latency test: single candle through the full pipeline must be < 100 ms
    /// (realistically it should be nanoseconds, but 100 ms is the hard cap).
    #[test]
    fn latency_single_candle_under_100ms() {
        let mut pipeline: Pipeline<50> = Pipeline::new(9, 21, 14);
        // Warm up
        for i in 0..20 {
            pipeline.feed(make_candle(i));
        }

        let start  = Instant::now();
        pipeline.feed(make_candle(21));
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 100,
            "Single candle latency {}ms ≥ 100ms", elapsed.as_millis()
        );
    }

    /// Throughput test: 100 000 candles must complete in < 1 s with no OOM.
    #[test]
    fn throughput_100k_candles_no_lag() {
        let mut pipeline: Pipeline<50> = Pipeline::new(9, 21, 14);
        let n = 100_000_usize;

        let start = Instant::now();
        for i in 0..n {
            pipeline.feed(make_candle(i));
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_secs() < 1,
            "100k candles took {}ms — exceeds 1 000ms budget", elapsed.as_millis()
        );
        // Buffer must stay at capacity (no unbounded growth)
        assert_eq!(pipeline.history().len(), 50);
    }

    /// Memory stability: history ring buffer length must not exceed HIST.
    #[test]
    fn ring_buffer_does_not_grow_unboundedly() {
        let mut pipeline: Pipeline<50> = Pipeline::new(9, 21, 14);
        for i in 0..10_000 {
            pipeline.feed(make_candle(i));
        }
        assert!(pipeline.history().len() <= 50);
    }
}
