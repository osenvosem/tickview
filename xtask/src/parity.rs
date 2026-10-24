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
pub struct Source {
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

/// Repository root, resolved at compile time so tests work from any CWD.
#[must_use]
pub fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf()
}

/// Anchor count for a TV tool: a fixed count, or a variable mode name
/// (`freehand`, `open-path`).
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Anchors {
    Count(#[allow(dead_code)] u8),
    Mode(#[allow(dead_code)] String),
}

/// One row of the extracted TradingView drawing catalog.
#[derive(serde::Deserialize)]
pub struct TvTool {
    pub tv_id: String,
    #[allow(dead_code)]
    pub label: String,
    #[allow(dead_code)]
    pub group: String,
    #[allow(dead_code)]
    pub section: String,
    #[allow(dead_code)]
    pub shortcut: Option<String>,
    #[allow(dead_code)]
    pub anchors: Anchors,
    #[allow(dead_code)]
    pub interaction: String,
    pub scope: String,
    #[allow(dead_code)] // reason lives in the manifest Entry; TV side is documentation
    pub exclusion_reason: Option<String>,
}

/// The extracted TV drawing catalog (`docs/evidence/tv/drawings-catalog.json`).
#[derive(serde::Deserialize)]
pub struct TvCatalog {
    #[allow(dead_code)]
    pub schema_version: u32,
    #[allow(dead_code)]
    pub source: Source,
    pub tools: Vec<TvTool>,
}

/// Parse the extracted TV drawing catalog JSON.
///
/// # Errors
/// Returns a message when the JSON does not match the catalog shape.
pub fn parse_tv_catalog(json: &str) -> Result<TvCatalog, String> {
    serde_json::from_str(json).map_err(|error| format!("invalid TV catalog JSON: {error}"))
}

/// Parse `DrawingKind` variant names from the tickchart source text.
#[must_use]
pub fn parse_kind_variants(src: &str) -> Vec<String> {
    parse_enum_variants(src, "pub enum DrawingKind {")
}

/// Parse `TvId` variant names from the ta registry source text.
#[must_use]
pub fn parse_tv_ids(src: &str) -> Vec<String> {
    parse_enum_variants(src, "pub enum TvId {")
}

fn parse_enum_variants(src: &str, head: &str) -> Vec<String> {
    let Some(start) = src.find(head) else {
        return Vec::new();
    };
    let body = &src[start + head.len()..];
    let end = body.find('}').unwrap_or(body.len());
    body[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('#'))
        .map(|line| line.trim_end_matches(',').to_owned())
        .filter(|name| name.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        .collect()
}

/// One row of the extracted TV study catalog
/// (`docs/evidence/tv/studies-catalog.json`).
#[derive(serde::Deserialize)]
pub struct TvStudy {
    pub tv_id: String,
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub tickview_id: Option<String>,
    pub scope: String,
    #[allow(dead_code)]
    pub exclusion_reason: Option<String>,
    #[allow(dead_code)]
    pub default_inputs: serde_json::Value,
    pub is_price_study: Option<bool>,
    #[allow(dead_code)]
    pub source: String,
}

#[derive(serde::Deserialize)]
pub struct TvStudiesCatalog {
    #[allow(dead_code)]
    pub schema_version: u32,
    #[allow(dead_code)]
    pub source: serde_json::Value,
    pub tools: Vec<TvStudy>,
}

/// Parse the extracted TV study catalog JSON.
///
/// # Errors
/// Returns a message when the JSON does not match the catalog shape.
pub fn parse_tv_studies(json: &str) -> Result<TvStudiesCatalog, String> {
    serde_json::from_str(json).map_err(|error| format!("invalid TV studies catalog JSON: {error}"))
}

/// Study-side validation: implemented-id coverage, TV-catalog coverage,
/// verified coverage (>=2 tests, >=1 evidence), and pane consistency against
/// the bundle's `is_price_study`. Coverage rules are skipped while the
/// manifest carries no study entries (pre-M3 manifests stay valid).
#[must_use]
pub fn validate_studies(
    catalog_json: &str,
    tv_studies_json: &str,
    ids: &[String],
    evidence_exists: &impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut errors = validate_str(catalog_json, evidence_exists);
    let Ok(catalog) = serde_json::from_str::<Catalog>(catalog_json) else {
        return errors;
    };
    if catalog.studies.is_empty() {
        return errors;
    }
    let tv = match parse_tv_studies(tv_studies_json) {
        Ok(tv) => tv,
        Err(error) => {
            errors.push(error);
            return errors;
        }
    };

    let tickview_ids: HashSet<&str> = catalog
        .studies
        .iter()
        .map(|entry| entry.tickview_id.as_str())
        .collect();
    for id in ids {
        if !tickview_ids.contains(id.as_str()) {
            errors.push(format!("implemented study {id} has no manifest entry"));
        }
    }

    let manifest_tv_ids: HashSet<&str> = catalog
        .studies
        .iter()
        .map(|entry| entry.tv_id.as_str())
        .collect();
    for tool in &tv.tools {
        match tool.scope.as_str() {
            "in-scope" => {
                if !manifest_tv_ids.contains(tool.tv_id.as_str()) {
                    errors.push(format!(
                        "in-scope TV study {} has no manifest entry",
                        tool.tv_id
                    ));
                }
            }
            "excluded" => match catalog.studies.iter().find(|e| e.tv_id == tool.tv_id) {
                Some(entry) if entry.status == "excluded" => {}
                Some(_) => {
                    errors.push(format!(
                        "TV study {} is excluded but not excluded",
                        tool.tv_id
                    ));
                }
                None => {
                    errors.push(format!(
                        "excluded TV study {} has no manifest entry",
                        tool.tv_id
                    ));
                }
            },
            other => errors.push(format!("{}: unknown scope {other:?}", tool.tv_id)),
        }
    }

    let tv_catalog_ids: HashSet<&str> = tv.tools.iter().map(|tool| tool.tv_id.as_str()).collect();
    for entry in &catalog.studies {
        if !tv_catalog_ids.contains(entry.tv_id.as_str()) {
            errors.push(format!("{}: tv_id not in TV studies catalog", entry.tv_id));
        }
    }

    for entry in &catalog.studies {
        if entry.status == "verified" {
            if entry.tests.len() < 2 {
                errors.push(format!("{}: verified studies need >=2 tests", entry.tv_id));
            }
            if entry.evidence.is_empty() {
                errors.push(format!("{}: verified studies need evidence", entry.tv_id));
            }
        }
    }

    for tool in &tv.tools {
        let Some(is_price) = tool.is_price_study else {
            continue;
        };
        let Some(entry) = catalog.studies.iter().find(|e| e.tv_id == tool.tv_id) else {
            continue;
        };
        if is_price && entry.pane != "price" {
            errors.push(format!(
                "{}: pane {:?} but TV marks it a price study",
                entry.tv_id, entry.pane
            ));
        }
        if !is_price && entry.pane == "price" {
            errors.push(format!(
                "{}: pane price but TV marks it non-price",
                entry.tv_id
            ));
        }
    }
    errors
}

/// Full validation: structural rules plus TV-catalog coverage, implemented-kind
/// coverage, and verified-drawing coverage (≥3 tests, ≥2 evidence incl. png).
#[must_use]
pub fn validate_full(
    catalog_json: &str,
    tv_json: &str,
    kinds: &[String],
    evidence_exists: &impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut errors = validate_str(catalog_json, evidence_exists);
    let Ok(catalog) = serde_json::from_str::<Catalog>(catalog_json) else {
        return errors;
    };
    let tv = match parse_tv_catalog(tv_json) {
        Ok(tv) => tv,
        Err(error) => {
            errors.push(error);
            return errors;
        }
    };

    let tickview_ids: HashSet<&str> = catalog
        .drawings
        .iter()
        .map(|entry| entry.tickview_id.as_str())
        .collect();
    for kind in kinds {
        if !tickview_ids.contains(kind.as_str()) {
            errors.push(format!(
                "implemented drawing kind {kind} has no manifest entry"
            ));
        }
    }

    let manifest_tv_ids: HashSet<&str> = catalog
        .drawings
        .iter()
        .map(|entry| entry.tv_id.as_str())
        .collect();
    for tool in &tv.tools {
        match tool.scope.as_str() {
            "in-scope" => {
                if !manifest_tv_ids.contains(tool.tv_id.as_str()) {
                    errors.push(format!(
                        "in-scope TV tool {} has no manifest entry",
                        tool.tv_id
                    ));
                }
            }
            "excluded" => match catalog.drawings.iter().find(|e| e.tv_id == tool.tv_id) {
                Some(entry) if entry.status == "excluded" => {}
                Some(_) => {
                    errors.push(format!(
                        "TV tool {} is excluded but not excluded",
                        tool.tv_id
                    ));
                }
                None => {
                    errors.push(format!(
                        "excluded TV tool {} has no manifest entry",
                        tool.tv_id
                    ));
                }
            },
            other => errors.push(format!("{}: unknown scope {other:?}", tool.tv_id)),
        }
    }

    let tv_catalog_ids: HashSet<&str> = tv.tools.iter().map(|tool| tool.tv_id.as_str()).collect();
    for entry in &catalog.drawings {
        if !tv_catalog_ids.contains(entry.tv_id.as_str()) {
            errors.push(format!("{}: tv_id not in TV catalog", entry.tv_id));
        }
    }

    for entry in &catalog.drawings {
        if entry.status == "verified" {
            if entry.tests.len() < 3 {
                errors.push(format!("{}: verified drawings need >=3 tests", entry.tv_id));
            }
            if entry.evidence.len() < 2 || !entry.evidence.iter().any(|p| p.ends_with(".png")) {
                errors.push(format!(
                    "{}: verified drawings need >=2 evidence incl. a .png",
                    entry.tv_id
                ));
            }
        }
    }
    errors
}

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
    let root = workspace_root();
    let path = root.join("docs/parity/catalog.json");
    let json = match std::fs::read_to_string(&path) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("cannot read {}: {error}", path.display());
            return 1;
        }
    };
    let tv_path = root.join("docs/evidence/tv/drawings-catalog.json");
    let tv_json = match std::fs::read_to_string(&tv_path) {
        Ok(tv) => tv,
        Err(error) => {
            eprintln!("cannot read {}: {error}", tv_path.display());
            return 1;
        }
    };
    let kinds_path = root.join("crates/tickchart/src/drawings.rs");
    let kinds_src = match std::fs::read_to_string(&kinds_path) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("cannot read {}: {error}", kinds_path.display());
            return 1;
        }
    };
    let kinds = parse_kind_variants(&kinds_src);
    let errors = validate_full(&json, &tv_json, &kinds, &|candidate| {
        root.join(candidate).exists() || Path::new(candidate).exists()
    });
    let tv_studies_path = root.join("docs/evidence/tv/studies-catalog.json");
    let tv_studies_json = match std::fs::read_to_string(&tv_studies_path) {
        Ok(tv) => tv,
        Err(error) => {
            eprintln!("cannot read {}: {error}", tv_studies_path.display());
            return 1;
        }
    };
    let registry_path = root.join("crates/ta/src/registry.rs");
    let registry_src = match std::fs::read_to_string(&registry_path) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("cannot read {}: {error}", registry_path.display());
            return 1;
        }
    };
    let study_ids = parse_tv_ids(&registry_src);
    let mut errors = errors;
    errors.extend(validate_studies(
        &json,
        &tv_studies_json,
        &study_ids,
        &|candidate| root.join(candidate).exists() || Path::new(candidate).exists(),
    ));
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
    use super::{
        TvTool, parse_kind_variants, parse_tv_catalog, parse_tv_ids, validate_full, validate_str,
        validate_studies, workspace_root,
    };

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

    #[test]
    fn tv_catalog_has_92_ids_and_one_excluded() {
        let path = workspace_root().join("docs/evidence/tv/drawings-catalog.json");
        let json = std::fs::read_to_string(path).expect("TV drawing catalog exists");
        let catalog = parse_tv_catalog(&json).expect("TV catalog parses");
        // 87 LineTools + FontIcon + 4 cursor rows; milestone 5 brought
        // Post/Idea in scope, leaving Image (media) and the duplicate arrow
        // cursor excluded.
        assert_eq!(catalog.tools.len(), 92);
        let excluded: Vec<&TvTool> = catalog
            .tools
            .iter()
            .filter(|tool| tool.scope == "excluded")
            .collect();
        let excluded_ids: Vec<&str> = excluded.iter().map(|tool| tool.tv_id.as_str()).collect();
        assert_eq!(excluded_ids, ["arrow"]);
        assert!(
            excluded.iter().all(|tool| !tool
                .exclusion_reason
                .as_deref()
                .unwrap_or_default()
                .is_empty()),
            "excluded rows need reasons"
        );
        let mut ids: Vec<&str> = catalog
            .tools
            .iter()
            .map(|tool| tool.tv_id.as_str())
            .collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate tv_id in TV catalog");
    }

    fn tv_json(rows: &[String]) -> String {
        format!(
            r#"{{"schema_version":1,"source":{{"app_version":"3.4.1","captured":"2026-10-20"}},"tools":[{}]}}"#,
            rows.join(",")
        )
    }

    fn tv_row(id: &str, scope: &str) -> String {
        format!(
            r#"{{"tv_id":"{id}","label":"L","group":"G","section":"S","shortcut":null,"anchors":2,"interaction":"two-point","scope":"{scope}","exclusion_reason":null}}"#
        )
    }

    fn empty_catalog() -> String {
        r#"{"schema_version":1,"source":{"app_version":"3.4.1","captured":"2026-10-20"},"drawings":[],"studies":[]}"#
            .to_owned()
    }

    #[test]
    fn parse_kind_variants_finds_gann_fan() {
        let src = "pub enum DrawingKind {\n    TrendLine,\n    GannFan,\n}\n";
        assert_eq!(parse_kind_variants(src), vec!["TrendLine", "GannFan"]);
    }

    #[test]
    fn implemented_kind_without_entry_is_error() {
        let errors = validate_full(
            &empty_catalog(),
            &tv_json(&[]),
            &["TrendLine".to_owned()],
            &evidence,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("implemented drawing kind TrendLine has no manifest entry")),
            "{errors:?}"
        );
    }

    #[test]
    fn in_scope_tv_id_without_entry_is_error() {
        let errors = validate_full(
            &empty_catalog(),
            &tv_json(&[tv_row("LineToolX", "in-scope")]),
            &[],
            &evidence,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("in-scope TV tool LineToolX has no manifest entry")),
            "{errors:?}"
        );
    }

    #[test]
    fn excluded_tv_id_must_be_excluded_status() {
        let json = with_drawing(
            r#"{ "kind": "drawing", "tv_id": "A", "tickview_id": "A", "name": "a",
                "icon": "ia", "status": "missing" }"#,
        );
        let errors = validate_full(&json, &tv_json(&[tv_row("A", "excluded")]), &[], &evidence);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("TV tool A is excluded but not excluded")),
            "{errors:?}"
        );
    }

    #[test]
    fn verified_drawing_needs_coverage() {
        let json = with_drawing(
            r#"{ "kind": "drawing", "tv_id": "A", "tickview_id": "A", "name": "a",
                "icon": "ia", "status": "verified", "tests": ["t"],
                "evidence": ["docs/evidence/tv/gann-fan.md"] }"#,
        );
        let errors = validate_full(&json, &tv_json(&[tv_row("A", "in-scope")]), &[], &evidence);
        assert!(
            errors.iter().any(|e| e.contains("need >=3 tests")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("need >=2 evidence incl. a .png")),
            "{errors:?}"
        );
    }

    #[test]
    fn real_manifest_covers_every_kind_and_tv_id() {
        let root = workspace_root();
        let catalog =
            std::fs::read_to_string(root.join("docs/parity/catalog.json")).expect("manifest");
        let tv = std::fs::read_to_string(root.join("docs/evidence/tv/drawings-catalog.json"))
            .expect("TV catalog");
        let kinds_src = std::fs::read_to_string(root.join("crates/tickchart/src/drawings.rs"))
            .expect("drawings.rs");
        let kinds = parse_kind_variants(&kinds_src);
        assert_eq!(kinds.len(), 91, "kind parse drift");
        let errors = validate_full(&catalog, &tv, &kinds, &|candidate| {
            root.join(candidate).exists()
        });
        assert!(errors.is_empty(), "{errors:#?}");
    }

    fn tv_studies(rows: &str) -> String {
        format!(
            r#"{{"schema_version":1,"source":{{"target":"3.4.1","wire":"w","meta_dir":"m"}},
              "tools":[{rows}]}}"#
        )
    }

    fn with_study(fragment: &str) -> String {
        format!(
            r#"{{"schema_version":1,"source":{{"app_version":"3.4.1","captured":"2026-10-20"}},
              "drawings":[],"studies":[{fragment}]}}"#
        )
    }

    const RSIS: &str = r#"{
        "kind":"study","tv_id":"RSI@tv-basicstudies","tickview_id":"Rsi",
        "name":"Relative Strength Index (RSI)","pane":"separate","status":"verified",
        "tests":["a::rsi_golden","b::rsi_defaults"],"evidence":["docs/evidence/tv/studies/x.json"],
        "notes":""}"#;

    const TV_RSI: &str = r#"{
        "tv_id":"RSI@tv-basicstudies","name":"Relative Strength Index (RSI)",
        "tickview_id":"Rsi","scope":"in-scope","exclusion_reason":null,
        "default_inputs":{"in_0":14},"is_price_study":false,"source":"wire"}"#;

    #[test]
    fn every_study_variant_without_entry_is_error() {
        let ids = ["Rsi".to_owned(), "Aroon".to_owned()];
        let errors = validate_studies(&with_study(RSIS), &tv_studies(TV_RSI), &ids, &|_| true);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("Aroon") && e.contains("no manifest entry")),
            "{errors:#?}"
        );
    }

    #[test]
    fn study_in_scope_tv_id_without_entry_is_error() {
        let ids = ["Rsi".to_owned()];
        let extra = r#"{
            "tv_id":"STD;Aroon","name":"Aroon","tickview_id":null,"scope":"in-scope",
            "exclusion_reason":null,"default_inputs":{},"is_price_study":false,"source":"meta"}"#;
        let errors = validate_studies(
            &with_study(RSIS),
            &tv_studies(&format!("{TV_RSI},{extra}")),
            &ids,
            &|_| true,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("STD;Aroon") && e.contains("no manifest entry")),
            "{errors:#?}"
        );
    }

    #[test]
    fn study_excluded_must_have_reason() {
        let entry = r#"{
            "kind":"study","tv_id":"STD;Advance_Decline_Ratio_Bars",
            "tickview_id":"AdvanceDecline","name":"Advance/Decline",
            "pane":"separate","status":"excluded","tests":[],"evidence":[],"notes":""}"#;
        let tv = r#"{
            "tv_id":"STD;Advance_Decline_Ratio_Bars","name":"Advance/Decline",
            "tickview_id":null,"scope":"excluded","exclusion_reason":"breadth",
            "default_inputs":{},"is_price_study":false,"source":"meta"}"#;
        let errors = validate_studies(&with_study(entry), &tv_studies(tv), &[], &|_| true);
        assert!(errors.iter().any(|e| e.contains("reason")), "{errors:#?}");
    }

    #[test]
    fn verified_study_needs_coverage() {
        let thin = r#"{
            "kind":"study","tv_id":"RSI@tv-basicstudies","tickview_id":"Rsi",
            "name":"RSI","pane":"separate","status":"verified",
            "tests":["only::one"],"evidence":[],"notes":""}"#;
        let errors = validate_studies(
            &with_study(thin),
            &tv_studies(TV_RSI),
            &["Rsi".to_owned()],
            &|_| true,
        );
        assert!(errors.iter().any(|e| e.contains("tests")), "{errors:#?}");
        assert!(errors.iter().any(|e| e.contains("evidence")), "{errors:#?}");
        assert!(
            validate_studies(
                &with_study(RSIS),
                &tv_studies(TV_RSI),
                &["Rsi".to_owned()],
                &|_| true
            )
            .is_empty()
        );
    }

    #[test]
    fn study_pane_drift_is_error() {
        let entry = r#"{
            "kind":"study","tv_id":"RSI@tv-basicstudies","tickview_id":"Rsi",
            "name":"RSI","pane":"price","status":"verified",
            "tests":["a::x","b::y"],"evidence":["docs/evidence/tv/studies/x.json"],"notes":""}"#;
        let errors = validate_studies(
            &with_study(entry),
            &tv_studies(TV_RSI),
            &["Rsi".to_owned()],
            &|_| true,
        );
        assert!(errors.iter().any(|e| e.contains("pane")), "{errors:#?}");
    }

    #[test]
    fn empty_studies_list_skips_coverage_rules() {
        assert!(
            validate_studies(
                r#"{"schema_version":1,"source":{"app_version":"3.4.1","captured":"x"},
               "drawings":[],"studies":[]}"#,
                &tv_studies(TV_RSI),
                &["Rsi".to_owned()],
                &|_| true
            )
            .is_empty()
        );
    }

    #[test]
    fn parse_tv_ids_finds_variants() {
        let src = "#[derive(Clone, Copy)]\npub enum TvId {\n    Sma,\n    Ema,\n    // note\n    Rsi,\n}\n";
        assert_eq!(parse_tv_ids(src), ["Sma", "Ema", "Rsi"]);
    }
}
