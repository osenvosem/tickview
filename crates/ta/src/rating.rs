//! Composite ratings mirroring the reference app's "Rating" summaries (chart-less).
//! Deterministic documented rules; UI renders Strong Buy … Strong Sell.

use crate::Candles;
use crate::series::{ema, sma};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub buy: u32,
    pub sell: u32,
    pub neutral: u32,
}

impl Score {
    #[must_use]
    pub fn total(&self) -> u32 {
        self.buy + self.sell + self.neutral
    }

    #[must_use]
    pub fn label(&self) -> RatingLabel {
        let total = self.total().max(1) as f64;
        let diff = self.buy as f64 - self.sell as f64;
        let frac = diff / total;
        if frac >= 0.75 {
            RatingLabel::StrongBuy
        } else if frac > 0.0 {
            RatingLabel::Buy
        } else if frac <= -0.75 {
            RatingLabel::StrongSell
        } else if frac < 0.0 {
            RatingLabel::Sell
        } else {
            RatingLabel::Neutral
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatingLabel {
    StrongBuy,
    Buy,
    Neutral,
    Sell,
    StrongSell,
}

#[must_use]
fn score_from(signals: &[i8]) -> Score {
    let mut s = Score {
        buy: 0,
        sell: 0,
        neutral: 0,
    };
    for &v in signals {
        match v {
            1 => s.buy += 1,
            -1 => s.sell += 1,
            _ => s.neutral += 1,
        }
    }
    s
}

/// Oscillator rating: RSI50, Stoch K/D, StochRSI K/D, CCI0, ADX(DI),
/// MACD histogram, Williams %R −50, ROC10, Momentum10.
#[must_use]
pub fn osc_rating(cd: &Candles) -> Score {
    let c = &cd.c;
    let (h, l) = (&cd.h, &cd.l);
    let n = c.len();
    if n < 200 {
        return Score {
            buy: 0,
            sell: 0,
            neutral: 9,
        };
    }
    let i = n - 1;
    let mut sig = [0i8; 9];
    let r = crate::oscs::rsi(c, 14);
    sig[0] = if r[i] > 50.0 {
        1
    } else if r[i] < 50.0 {
        -1
    } else {
        0
    };
    let st = crate::oscs::stochastic(h, l, c, 14, 3, 1);
    sig[1] = cmp2(st.k[i], st.d[i]);
    let sr = crate::oscs::stoch_rsi(c, 14, 14, 1, 3);
    sig[2] = cmp2(sr.k[i], sr.d[i]);
    let cci = crate::oscs::cci(cd, 20);
    sig[3] = sign(cci[i]);
    let adx = crate::oscs::adx(cd, 14, 14);
    sig[4] = if adx.di_plus[i] > adx.di_minus[i] {
        1
    } else if adx.di_plus[i] < adx.di_minus[i] {
        -1
    } else {
        0
    };
    let m = crate::oscs::macd(c, 12, 26, 9);
    sig[5] = sign(m.hist[i]);
    let wr = crate::oscs::williams_r(h, l, c, 14);
    sig[6] = if wr[i] > -50.0 {
        1
    } else if wr[i] < -50.0 {
        -1
    } else {
        0
    };
    sig[7] = sign(crate::series::roc(c, 10, true)[i]);
    sig[8] = sign(crate::oscs::momentum(c, 10, crate::oscs::MomMode::Abs)[i]);
    score_from(&sig)
}

/// Moving-average rating: price vs EMA/SMA 10, 20, 30, 50, 100, 200.
#[must_use]
pub fn ma_rating(cd: &Candles) -> Score {
    let c = &cd.c;
    let n = c.len();
    if n < 201 {
        return Score {
            buy: 0,
            sell: 0,
            neutral: 12,
        };
    }
    let i = n - 1;
    let mut sig = [0i8; 12];
    for (k, len) in [10usize, 20, 30, 50, 100, 200].iter().enumerate() {
        sig[k] = cmp2(c[i], ema(c, *len)[i]);
        sig[k + 6] = cmp2(c[i], sma(c, *len)[i]);
    }
    score_from(&sig)
}

/// Combined technical rating.
#[must_use]
pub fn tech_rating(cd: &Candles) -> Score {
    let o = osc_rating(cd);
    let m = ma_rating(cd);
    Score {
        buy: o.buy + m.buy,
        sell: o.sell + m.sell,
        neutral: o.neutral + m.neutral,
    }
}

#[must_use]
fn sign(v: f64) -> i8 {
    if v.is_finite() {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    } else {
        0
    }
}

#[must_use]
fn cmp2(a: f64, b: f64) -> i8 {
    if a.is_finite() && b.is_finite() {
        sign(a - b)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bar;

    fn ramp(n: usize) -> Candles {
        let bars: Vec<Bar> = (0..n)
            .map(|i| {
                let c = 10.0 + (i as f64) * 0.1;
                Bar {
                    t: i as i64,
                    o: c,
                    h: c + 0.5,
                    l: c - 0.5,
                    c,
                    v: 100.0,
                }
            })
            .collect();
        Candles::from_bars(&bars)
    }

    #[test]
    fn uptrend_scores_buy_side() {
        let cd = ramp(250);
        let ma = ma_rating(&cd);
        assert_eq!(ma.buy, 12, "steady uptrend: all MAs below price");
        let o = osc_rating(&cd);
        assert!(o.buy > o.sell, "uptrend leans buy: {o:?}");
        assert_eq!(tech_rating(&cd).label(), RatingLabel::StrongBuy);
    }

    #[test]
    fn short_series_is_neutral() {
        let cd = ramp(10);
        assert_eq!(ma_rating(&cd).label(), RatingLabel::Neutral);
        assert_eq!(osc_rating(&cd).total(), 9);
    }
}
