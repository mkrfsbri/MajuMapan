use btc_sniper::types::Candle;
use btc_sniper::pipeline::Pipeline;

fn main() {
    println!("=== Ultra Sniper BTC Bot — Live Demo ===\n");

    let mut pipeline: Pipeline<50> = Pipeline::new(9, 21, 14);

    // Simulated OHLCV feed
    let prices: &[f64] = &[
        29_500.0, 29_800.0, 30_100.0, 30_050.0, 30_300.0,
        30_600.0, 30_400.0, 30_700.0, 31_000.0, 30_800.0,
        30_500.0, 30_200.0, 29_900.0, 29_600.0, 29_400.0,
        29_700.0, 30_000.0, 30_350.0, 30_650.0, 31_100.0,
    ];

    for (i, &price) in prices.iter().enumerate() {
        let candle = Candle::new(price, price + 50.0, price - 50.0, price + 10.0, 1.0);
        let out    = pipeline.feed(candle);

        println!(
            "Bar {:>2} | close={:.0} | EMA9={:.2} | EMA21={:.2} | RSI={} | Signal={:?}",
            i + 1,
            price,
            out.ema_fast,
            out.ema_slow,
            out.rsi.map_or("--".into(), |r| format!("{r:.1}")),
            out.signal,
        );
    }

    println!("\nAll bars processed. No panics.");
}
