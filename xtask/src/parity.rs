//! Parity manifest validation: `docs/parity/catalog.json` → `docs/parity/STATUS.md`.
//!
//! The catalog is the machine-readable record of TradingView feature parity.
//! `xtask parity` validates it; `xtask parity --write` also regenerates the
//! human-readable status tables.

use std::collections::HashSet;
use std::path::Path;

/// Top-level manifest document.
#[derive(serde::Deserialize)]
struct Catalog {
    schema_version: u32,
    #[allow(dead_code)]
    source: Source,
    #[serde(default)]
    drawings: Vec<Entry>,
    #[serde(default)]
    studies: Vec<Entry>,
}

/// Provenance of the reference capture the manifest was seeded from.
#[derive(serde::Deserialize)]
struct Source {
    #[allow(dead_code)]
    app_version: String,
    #[allow(dead_code)]
    captured: String,
}

/// One catalog row (a drawing tool or a study).
#[derive(serde::Deserialize)]
struct Entry {
    kind: String,
    tv_id: String,
    tickview_id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    group: String,
    #[serde(default)]
    #[allow(dead_code)]
    pane: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    #[allow(dead_code)]
    anchors: u8,
    #[serde(default)]
    #[allow(dead_code)]
    interaction: String,
    status: String,
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    notes: String,
    #[serde(default)]
    exclusion_reason: Option<String>,
}

const VALID_STATUS: [&str; 4] = ["missing", "in-progress", "verified", "excluded"];

/// Validate a manifest string. `evidence_exists` decides whether an evidence
/// path resolves (the real command checks the filesystem; tests fake it).
#[must_use]
pub fn validate_str(json: &str, evidence_exists: impl Fn(&str) -> bool) -> Vec<String> {
    let catalog: Catalog = match serde_json::from_str(json) {
        Ok(catalog) => catalog,
        Err(error) => return vec![format!("invalid JSON: {error}")],
    };
    validate_catalog(&catalog, &evidence_exists)
}

fn validate_catalog(catalog: &Catalog, evidence_exists: &impl Fn(&str) -> bool) -> Vec<String> {
    let mut errors = Vec::new();

    if catalog.schema_version != 1 {
        errors.push(format!(
            "unsupported schema_version {} (expected 1)",
            catalog.schema_version
        ));
    }

    let mut tv_ids = HashSet::new();
    let mut tickview_ids = HashSet::new();
    let mut icons = HashSet::new();

    for (list, expected_kind) in [(&catalog.drawings, "drawing"), (&catalog.studies, "study")] {
        for entry in list {
            let label = format!("{} ({})", entry.tv_id, entry.tickview_id);

            if entry.kind != expected_kind {
                errors.push(format!(
                    "{label}: kind {:?} does not match list",
                    entry.kind
                ));
            }
            if !VALID_STATUS.contains(&entry.status.as_str()) {
                errors.push(format!("{label}: unknown status {:?}", entry.status));
            }
            if !tv_ids.insert(entry.tv_id.clone()) {
                errors.push(format!("{label}: duplicate tv_id {}", entry.tv_id));
            }
            if !tickview_ids.insert(entry.tickview_id.clone()) {
                errors.push(format!(
                    "{label}: duplicate tickview_id {}",
                    entry.tickview_id
                ));
            }
            if expected_kind == "drawing" {
                if entry.icon.is_empty() {
                    errors.push(format!("{label}: drawing entries need an icon"));
                } else if !icons.insert(entry.icon.clone()) {
                    errors.push(format!("{label}: duplicate icon {}", entry.icon));
                }
            }
            if entry.status == "verified" {
                if entry.tests.is_empty() {
                    errors.push(format!("{label}: verified entries need at least one test"));
                }
                for path in &entry.evidence {
                    if !evidence_exists(path) {
                        errors.push(format!("{label}: missing evidence path {path}"));
                    }
                }
            }
            if entry.status == "excluded"
                && entry.exclusion_reason.as_deref().is_none_or(str::is_empty)
            {
                errors.push(format!("{label}: excluded entries need a reason"));
            }
        }
    }

    errors
}

/// Render the human-readable `docs/parity/STATUS.md`.
pub fn render_status(json: &str) -> Result<String, Vec<String>> {
    let catalog: Catalog =
        serde_json::from_str(json).map_err(|error| vec![format!("invalid JSON: {error}")])?;
    let counts = |entries: &[Entry]| -> [usize; 4] {
        let mut counts = [0usize; 4];
        for entry in entries {
            if let Some(index) = VALID_STATUS
                .iter()
                .position(|status| *status == entry.status)
            {
                counts[index] += 1;
            }
        }
        counts
    };
    let drawings = counts(&catalog.drawings);
    let studies = counts(&catalog.studies);

    let mut out = String::new();
    out.push_str("# TickView ↔ TradingView parity status\n\n");
    out.push_str(&format!(
        "Source: TradingView {} (captured {}).\n\n",
        catalog.source.app_version, catalog.source.captured
    ));
    out.push_str(&format!(
        "Drawings: {} verified, {} in progress, {} missing, {} excluded ({} total).\n",
        drawings[2],
        drawings[1],
        drawings[0],
        drawings[3],
        catalog.drawings.len()
    ));
    out.push_str(&format!(
        "Studies: {} verified, {} in progress, {} missing, {} excluded ({} total).\n\n",
        studies[2],
        studies[1],
        studies[0],
        studies[3],
        catalog.studies.len()
    ));

    push_table(&mut out, "Drawings", &catalog.drawings);
    push_table(&mut out, "Studies", &catalog.studies);
    Ok(out)
}

fn push_table(out: &mut String, title: &str, entries: &[Entry]) {
    out.push_str(&format!("## {title}\n\n"));
    if entries.is_empty() {
        out.push_str("_None yet._\n\n");
        return;
    }
    out.push_str("| Name | TradingView id | TickView id | Icon | Status |\n");
    out.push_str("|---|---|---|---|---|\n");
    for entry in entries {
        out.push_str(&format!(
            "| {} | `{}` | `{}` | `{}` | {} |\n",
            entry.name, entry.tv_id, entry.tickview_id, entry.icon, entry.status
        ));
    }
    out.push('\n');
}

/// `xtask parity [--write]` entry point. Returns a process exit code.
pub fn run(args: &[String]) -> i32 {
    let write = args.iter().any(|arg| arg == "--write");
    let path = Path::new("docs/parity/catalog.json");
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("cannot read {}: {error}", path.display());
            return 1;
        }
    };
    let errors = validate_str(&json, |candidate| Path::new(candidate).exists());
    if !errors.is_empty() {
        for error in &errors {
            eprintln!("parity: {error}");
        }
        return 1;
    }
    if write {
        match render_status(&json) {
            Ok(status) => {
                if let Err(error) = std::fs::write("docs/parity/STATUS.md", status) {
                    eprintln!("cannot write STATUS.md: {error}");
                    return 1;
                }
                println!("wrote docs/parity/STATUS.md");
            }
            Err(render_errors) => {
                for error in render_errors {
                    eprintln!("parity: {error}");
                }
                return 1;
            }
        }
    }
    println!("parity manifest is valid");
    0
}

#[cfg(test)]
mod tests {
    use super::validate_str;

    /// Only the Gann evidence path "exists" in tests.
    fn evidence(path: &str) -> bool {
        path == "docs/evidence/tv/gann-fan.md"
    }

    const VALID: &str = r#"{
        "schema_version": 1,
        "source": { "app_version": "3.4.1", "captured": "2026-10-20" },
        "drawings": [
            {
                "kind": "drawing",
                "tv_id": "LineToolGannFan",
                "tickview_id": "GannFan",
                "name": "Gann fan",
                "group": "Gann and Fibonacci tools",
                "icon": "gann-fan",
                "anchors": 2,
                "interaction": "two-point",
                "status": "in-progress",
                "tests": [],
                "evidence": ["docs/evidence/tv/gann-fan.md"],
                "notes": "Seed entry."
            }
        ],
        "studies": []
    }"#;

    fn with_drawing(fragment: &str) -> String {
        format!(
            r#"{{"schema_version":1,"source":{{"app_version":"3.4.1","captured":"2026-10-20"}},
              "drawings":[{fragment}],"studies":[]}}"#
        )
    }

    #[test]
    fn valid_manifest_produces_no_errors() {
        assert_eq!(validate_str(VALID, evidence), Vec::<String>::new());
    }

    #[test]
    fn duplicate_tickview_id_is_rejected() {
        let json = r#"{
            "schema_version": 1,
            "source": { "app_version": "3.4.1", "captured": "2026-10-20" },
            "drawings": [
                { "kind": "drawing", "tv_id": "A", "tickview_id": "Dup", "name": "a", "icon": "ia", "status": "missing" },
                { "kind": "drawing", "tv_id": "B", "tickview_id": "Dup", "name": "b", "icon": "ib", "status": "missing" }
            ],
            "studies": []
        }"#;
        let errors = validate_str(json, evidence);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("duplicate tickview_id Dup")),
            "{errors:?}"
        );
    }

    #[test]
    fn duplicate_icon_is_rejected() {
        let json = r#"{
            "schema_version": 1,
            "source": { "app_version": "3.4.1", "captured": "2026-10-20" },
            "drawings": [
                { "kind": "drawing", "tv_id": "A", "tickview_id": "A", "name": "a", "icon": "same", "status": "missing" },
                { "kind": "drawing", "tv_id": "B", "tickview_id": "B", "name": "b", "icon": "same", "status": "missing" }
            ],
            "studies": []
        }"#;
        let errors = validate_str(json, evidence);
        assert!(
            errors.iter().any(|e| e.contains("duplicate icon same")),
            "{errors:?}"
        );
    }

    #[test]
    fn verified_entry_without_tests_is_rejected() {
        let json = with_drawing(
            r#"{ "kind": "drawing", "tv_id": "A", "tickview_id": "A", "name": "a",
                "icon": "ia", "status": "verified", "evidence": ["docs/evidence/tv/gann-fan.md"] }"#,
        );
        let errors = validate_str(&json, evidence);
        assert!(
            errors.iter().any(|e| e.contains("need at least one test")),
            "{errors:?}"
        );
    }

    #[test]
    fn verified_entry_with_missing_evidence_is_rejected() {
        let json = with_drawing(
            r#"{ "kind": "drawing", "tv_id": "A", "tickview_id": "A", "name": "a",
                "icon": "ia", "status": "verified", "tests": ["t"],
                "evidence": ["docs/evidence/tv/nope.md"] }"#,
        );
        let errors = validate_str(&json, evidence);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("missing evidence path docs/evidence/tv/nope.md")),
            "{errors:?}"
        );
    }

    #[test]
    fn excluded_entry_without_reason_is_rejected() {
        let json = with_drawing(
            r#"{ "kind": "drawing", "tv_id": "A", "tickview_id": "A", "name": "a",
                "icon": "ia", "status": "excluded" }"#,
        );
        let errors = validate_str(&json, evidence);
        assert!(
            errors.iter().any(|e| e.contains("need a reason")),
            "{errors:?}"
        );
    }
}
