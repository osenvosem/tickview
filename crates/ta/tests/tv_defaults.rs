//! Pin registry defaults against TradingView's `studies_metadata` wire
//! payload (evidence committed at `docs/evidence/tv-studies-defaults.json`,
//! captured from TradingView Desktop 3.4.1.8194 packs tv-basicstudies /
//! tv-corestudies / tv-prostudies). Numerical and boolean defaults must
//! match; enum inputs are recorded in the evidence for manual review.

use std::collections::BTreeMap;

use ta::registry::{ParamKind, defs};

fn norm(s: &str) -> String {
    let mut out: String = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_lowercase();
    out = out.replace("length", "len");
    out = out.replace("signallen", "siglen");
    out = match out.as_str() {
        "klen" => "length".to_owned(),
        "dlen" => "signal".to_owned(),
        other => other.to_owned(),
    };
    match out.as_str() {
        "max" => "maxvalue".to_owned(),
        "inc" => "increment".to_owned(),
        other => other.to_owned(),
    }
}

#[test]
fn registry_defaults_match_tv_wire() {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/evidence/tv-studies-defaults.json"
    ))
    .expect("committed evidence");
    let evidence: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&raw).expect("valid evidence json");
    assert!(
        evidence
            .values()
            .filter(|v| v["wire_id"].is_string())
            .count()
            >= 40,
        "audit must cover at least 40 studies"
    );

    let mut drift: Vec<String> = Vec::new();
    let mut compared = 0usize;
    for def in defs() {
        let entry = &evidence[&format!("{:?}", def.id)];
        if entry["missing"].as_bool() == Some(true) {
            continue;
        }
        let inputs = entry["inputs"].as_object().expect("inputs object");
        let wire: BTreeMap<String, &serde_json::Value> =
            inputs.iter().map(|(k, v)| (norm(k), v)).collect();
        for p in def.params {
            let cand = [norm(p.key), norm(p.name)];
            let Some(wkey) = cand.iter().find(|c| wire.contains_key(*c)) else {
                continue;
            };
            let wv = wire[wkey];
            match p.kind {
                ParamKind::Int { def: d, .. } => {
                    compared += 1;
                    let w = wv.as_f64().unwrap_or_else(|| {
                        panic!("{:?} {}: wire value {wv} is not numeric", def.id, p.key)
                    });
                    if (w - d as f64).abs() > 1e-9 {
                        drift.push(format!(
                            "{:?} {}: registry {}, tv wire {w}",
                            def.id, p.key, d as f64
                        ));
                    }
                }
                ParamKind::Float { def: d, .. } => {
                    compared += 1;
                    let w = wv.as_f64().unwrap_or_else(|| {
                        panic!("{:?} {}: wire value {wv} is not numeric", def.id, p.key)
                    });
                    if (w - d).abs() > 1e-9 {
                        drift.push(format!("{:?} {}: registry {d}, tv wire {w}", def.id, p.key));
                    }
                }
                ParamKind::Bool { def: d } => {
                    compared += 1;
                    if let Some(w) = wv.as_bool()
                        && w != d
                    {
                        drift.push(format!("{:?} {}: registry {d}, tv wire {w}", def.id, p.key));
                    }
                }
                ParamKind::Enum { .. } => {}
            }
        }
    }
    assert!(
        drift.is_empty(),
        "registry defaults drift from TV wire:\n{}",
        drift.join("\n")
    );
    assert!(
        compared >= 50,
        "expected >=50 compared params, got {compared}"
    );
}
