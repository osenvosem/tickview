# Milestone 4 — Final Parity Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the completed milestone branches, regenerate every parity artifact, and run the final side-by-side review so the manifest, docs, screenshots, and gates all agree.

**Architecture:** Merge `feat/m3-study-catalog` into `main` (clean merge verified; base = M2 merge commit). All regeneration happens on `main`: STATUS from the manifest, an evidence index over `docs/evidence/`, fresh dark/light screenshots, then a written side-by-side review against TV reference evidence. No new product features — M4 only integrates, regenerates, and verifies.

**Tech Stack:** Rust workspace (`ta`, `tickchart`, `marketdata`, `app`/`tickview`, `xtask`), `cargo`, egui screenshot hook (`TICKVIEW_SNAPSHOT` + `TICKVIEW_CONFIG_DIR`), markdown docs.

**Spec:** `docs/superpowers/specs/2026-10-20-tradingview-full-parity-design.md` (§14 Milestone 4, §15 Acceptance Criteria)

## Global Constraints

- Gates: `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `git diff --check` — all clean.
- Manifest truth: `docs/parity/catalog.json` is the single source; `docs/parity/STATUS.md` is generated via `cargo run -p xtask -- parity --write`; hand-written docs must match it (drawings: 87 entries, 84 verified, 3 excluded; studies: 84 entries, 83 verified, 1 excluded).
- Every excluded entry carries a non-empty `exclusion_reason` (xtask validation enforces).
- Screenshots: 1440×900, dark + light, isolated config dirs (`TICKVIEW_CONFIG_DIR=$(mktemp -d)`), run `./target/debug/tickview` directly (not `cargo run`).
- No destructive git operations; preserve all user changes.

## Review Focus

- Manifest/docs drift after merge — STATUS, PARITY.md and README counts must equal the merged manifest; task 2 regenerates and task 5 diffs.
- Excluded-without-reason entries — xtask parity validation is the pin; task 2 runs it on the merged tree.
- Screenshots taken before data/comparison arrive — use the warm-cache two-pass pattern; task 3 verifies each image shows content, not a SYNCING frame.
- Dead actions in integrated UI — toolbar/menu coverage already pinned by `every_tool_is_wired_and_in_catalog` and `every_study_is_addable_and_in_catalog`; task 1 re-runs them post-merge.
- Reference drift — final review doc cites the specific TV evidence file per area; task 4.

---

### Task 1: Merge milestone 3 into main

**Files:**
- Modify: `main` (merge commit)

- [x] **Step 1:** `git checkout main && git merge --no-ff feat/m3-study-catalog -m "Merge branch 'feat/m3-study-catalog': milestone 3 study catalog parity"` (merge-tree dry-run already shows 0 conflicts).
- [x] **Step 2:** `cargo test --workspace` → PASS (343 + M2-era tests); `cargo clippy --workspace --all-targets` → 0 warnings.
- [x] **Step 3:** `cargo fmt --all` (no-op expected) && `git diff --check` clean; commit any residue.

### Task 2: Evidence index + regenerated status

**Files:**
- Create: `docs/evidence/INDEX.md`
- Modify: `docs/parity/STATUS.md` (generated)

- [x] **Step 1:** Write `docs/evidence/INDEX.md`: table of every file under `docs/evidence/` (and `docs/evidence/tv/`, `tv/studies/` count) with a one-line description each, plus a "how to regenerate" footer (`xtask parity --write`, `research/studies/gen_goldens.py`, `research/studies/build_catalog.py`).
- [x] **Step 2:** `cargo run -q -p xtask -- parity --write && cargo run -q -p xtask -- parity` → "parity manifest is valid"; STATUS shows drawings 84 verified/3 excluded and studies 83 verified/1 excluded, zero missing/in-progress.
- [x] **Step 3:** Commit — `git add docs/evidence docs/parity && git commit -m "docs: evidence index and regenerated parity status"`.

### Task 3: Fresh visual QA set on integrated main

**Files:**
- Create: `docs/screenshots/parity/{iconsheet,gannf,toolbar,trend,studiesprice,studiesvol,studiesosc,studieshtf}-{dark,light}.png` (regenerated)

- [x] **Step 1:** `cargo build -q -p tickview`; for each scene × theme: warm pass (no snapshot) then capture pass with isolated `TICKVIEW_CONFIG_DIR`; kill any leftover process between runs.
- [x] **Step 2:** Visually verify each image: bars + studies/overlays rendered, no SYNCING-empty frame, `studieshtf-*` shows comparison line + correlation curve in −1..1, toolbar shows selected-object bar, iconsheet shows the full icon grid.

### Task 4: Final side-by-side review doc

**Files:**
- Create: `docs/parity/final-review.md`

- [x] **Step 1:** Write `final-review.md`: one section per review area (drawing UI + selection toolbar, icon set, Gann fan geometry, representative studies — price/volume/oscillator/HTF-correlation), each citing the screenshot pair and the TV reference evidence file (`docs/evidence/tv/gann-fan.md`, `drawings-*.md`, `studies-catalog-notes.md`, `docs/evidence/tv-studies-defaults.json`), with an explicit verdict line (match / deviation + reason).
- [x] **Step 2:** Commit — `git add docs/screenshots/parity docs/parity/final-review.md && git commit -m "docs: milestone 4 final side-by-side parity review"`.

### Task 5: Final gate + acceptance checklist

**Files:**
- Modify: `README.md` (M4-complete wording, final test count)

- [x] **Step 1:** `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets && git diff --check` — all clean; record exact test count.
- [x] **Step 2:** README: replace "Milestone 3 covers/complete" tail with Milestone 4-complete paragraph + acceptance checklist result (manifest totals, gates, review doc path); update test count lines.
- [x] **Step 3:** `cargo run -q -p xtask -- parity` still valid; commit — `git commit -am "docs: milestone 4 complete — parity program final pass"`.

---

## Self-review notes

- Spec §14 M4 deliverables → tasks: regenerate docs/screenshots/evidence = t2/t3/t5; full regression gates = t1/t5; side-by-side review = t4. §15 acceptance lines all map: manifest totals (t2), icons (M1/M2 pinned tests re-run t1), Gann geometry (`drawing_goldens` + gannf shot t3), cursor/toolbar behavior (existing tests + toolbar shot t3), no dead items (wired/addable sweeps t1), 238-tests-remain-green superseded by current count (t5), gates (t1/t5).
- No new product code; no interfaces to define between tasks beyond file paths.
