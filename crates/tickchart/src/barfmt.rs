//! Adaptive price / compact number formatting for axes and legends.

/// Adaptive price formatting, reference-app-style.
#[must_use]
pub fn fmt_price(p: f64) -> String {
    if !p.is_finite() {
        return "—".into();
    }
    let a = p.abs();
    let dec = if a >= 1.0 {
        2
    } else if a >= 0.01 {
        4
    } else if a >= 0.0001 {
        6
    } else {
        8
    };
    let s = format!("{p:.dec$}");
    if a < 1.0 && s.contains('.') {
        let t = s.trim_end_matches('0');
        let t = t.trim_end_matches('.');
        return t.to_string();
    }
    s
}

/// Compact axis labels: 1.2k, 345M, 1.1B.
#[must_use]
pub fn fmt_compact(x: f64) -> String {
    if !x.is_finite() {
        return "—".into();
    }
    let a = x.abs();
    if a >= 1e9 {
        trim_tail(format!("{:.1}", x / 1e9)) + "B"
    } else if a >= 1e6 {
        trim_tail(format!("{:.1}", x / 1e6)) + "M"
    } else if a >= 1e4 {
        trim_tail(format!("{:.1}", x / 1e3)) + "k"
    } else if x == x.trunc() {
        format!("{}", x as i64)
    } else {
        let s = format!("{x:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn trim_tail(s: String) -> String {
    if s.ends_with(".0") {
        s[..s.len() - 2].to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_adaptive_decimals() {
        assert_eq!(fmt_price(123.456), "123.46");
        assert_eq!(fmt_price(0.5), "0.5");
        assert_eq!(fmt_price(0.00001234), "0.00001234");
        assert_eq!(fmt_price(f64::NAN), "—");
        assert_eq!(fmt_price(-7.1), "-7.10");
    }

    #[test]
    fn compact_labels() {
        assert_eq!(fmt_compact(1234.0), "1234"); // k-suffix only at >=10k, reference-style
        assert_eq!(fmt_compact(9999.0), "9999");
        assert_eq!(fmt_compact(1_500_000.0), "1.5M");
        assert_eq!(fmt_compact(2_000_000_000.0), "2B");
        assert_eq!(fmt_compact(55.5), "55.5");
        assert_eq!(fmt_compact(100.0), "100");
    }
}
