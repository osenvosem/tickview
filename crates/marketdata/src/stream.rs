//! Live websocket kline streams (Binance spot).

use std::time::{Duration, Instant};

use ta::Bar;
use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{WebSocket, connect};

use crate::error::DataError;
use crate::interval::{Interval, Unit};

fn field(k: &serde_json::Value, key: &str) -> Option<f64> {
    match k.get(key)? {
        serde_json::Value::String(s) => s.parse().ok(),
        serde_json::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// Parse a websocket frame into a `Bar`, but only when the kline is closed
/// (`k.x == true`). Open klines and non-kline events yield `None`.
#[must_use]
pub fn parse_final_kline(msg: &serde_json::Value) -> Option<Bar> {
    if msg.get("e").and_then(serde_json::Value::as_str) != Some("kline") {
        return None;
    }
    let k = msg.get("k")?;
    if k.get("x").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    Some(Bar {
        t: k.get("t")?.as_i64()? / 1000,
        o: field(k, "o")?,
        h: field(k, "h")?,
        l: field(k, "l")?,
        c: field(k, "c")?,
        v: field(k, "v")?,
    })
}

/// Binance `interval` parameter for a TickView interval. Binance offers no
/// second-level klines, so seconds degrade to `1m`.
#[must_use]
pub fn kline_interval(iv: Interval) -> String {
    match iv.unit {
        Unit::Sec => "1m".to_owned(),
        Unit::Min => format!("{}m", iv.mult),
        Unit::Hour => format!("{}h", iv.mult),
        Unit::Day => format!("{}d", iv.mult),
        Unit::Week => format!("{}w", iv.mult),
        Unit::Month => format!("{}M", iv.mult),
    }
}

/// Blocking websocket subscription to `<symbol>@kline_<interval>` on the
/// Binance spot stream host. Designed for a dedicated poller thread.
pub struct KlineStream {
    ws: WebSocket<MaybeTlsStream<std::net::TcpStream>>,
}

impl KlineStream {
    /// `symbol` is the raw Binance stream symbol (e.g. `btcusdt`).
    pub fn connect(symbol: &str, interval: &str) -> Result<Self, DataError> {
        let sym = symbol.to_ascii_lowercase();
        let url = format!("wss://stream.binance.com:9443/ws/{sym}@kline_{interval}");
        let (ws, _resp) =
            connect(&url).map_err(|e| DataError::Unavailable(format!("stream connect: {e}")))?;
        Ok(Self { ws })
    }

    /// Block until the next closed bar. `Ok(None)` means no frame arrived
    /// within `timeout`, which the caller treats as a liveness signal.
    pub fn next_final_bar(&mut self, timeout: Duration) -> Result<Option<Bar>, DataError> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1));
            if let Some(sock) = self.socket_mut() {
                sock.set_read_timeout(Some(left))
                    .map_err(|e| DataError::Unavailable(e.to_string()))?;
            }
            match self.ws.read() {
                Ok(Message::Text(txt)) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(txt.as_str())
                        && let Some(bar) = parse_final_kline(&v)
                    {
                        return Ok(Some(bar));
                    }
                }
                Ok(Message::Close(_)) => return Err(DataError::StreamClosed),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(DataError::Unavailable(format!("stream read: {e}"))),
            }
        }
    }

    fn socket_mut(&mut self) -> Option<&mut std::net::TcpStream> {
        match self.ws.get_mut() {
            MaybeTlsStream::Plain(s) => Some(s),
            MaybeTlsStream::Rustls(s) => Some(&mut s.sock),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interval::Interval;

    fn kline_msg(final_bar: bool) -> serde_json::Value {
        serde_json::json!({
            "e": "kline",
            "E": 1_700_000_000_123i64,
            "s": "BTCUSDT",
            "k": {
                "t": 1_700_000_000_000i64,
                "T": 1_700_000_059_999i64,
                "s": "BTCUSDT",
                "i": "1m",
                "o": "100.5",
                "h": "101.25",
                "l": "100.1",
                "c": "101.0",
                "v": "12.5",
                "x": final_bar
            }
        })
    }

    #[test]
    fn parses_closed_kline_into_second_bar() {
        let bar = parse_final_kline(&kline_msg(true)).expect("final kline");
        assert_eq!(bar.t, 1_700_000_000);
        assert!((bar.o - 100.5).abs() < 1e-9);
        assert!((bar.h - 101.25).abs() < 1e-9);
        assert!((bar.l - 100.1).abs() < 1e-9);
        assert!((bar.c - 101.0).abs() < 1e-9);
        assert!((bar.v - 12.5).abs() < 1e-9);
    }

    #[test]
    fn open_kline_and_other_events_are_ignored() {
        assert!(parse_final_kline(&kline_msg(false)).is_none());
        let trade = serde_json::json!({"e": "trade", "s": "BTCUSDT", "p": "1", "q": "1"});
        assert!(parse_final_kline(&trade).is_none());
    }

    #[test]
    fn kline_interval_suffixes_match_binance() {
        assert_eq!(&kline_interval(Interval::parse("15m").unwrap()), "15m");
        assert_eq!(&kline_interval(Interval::parse("1h").unwrap()), "1h");
        assert_eq!(&kline_interval(Interval::parse("D").unwrap()), "1d");
        assert_eq!(&kline_interval(Interval::parse("W").unwrap()), "1w");
        assert_eq!(&kline_interval(Interval::parse("M").unwrap()), "1M");
        // Binance has no second-level klines; degrade to 1 minute.
        assert_eq!(&kline_interval(Interval::parse("1s").unwrap()), "1m");
    }

    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_smoke_btcusdt_1m_stream() {
        let mut st = KlineStream::connect("btcusdt", "1m").expect("connect");
        for _ in 0..3 {
            let bar = st
                .next_final_bar(Duration::from_secs(70))
                .expect("read")
                .expect("closed bar");
            assert!(bar.c > 0.0);
            assert!(bar.h >= bar.l);
        }
    }
}
