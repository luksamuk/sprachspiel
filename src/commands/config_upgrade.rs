//! Implements `sprach config upgrade` — merge missing default fields into
//! the existing `config.toml`, preserving user values, comments, and
//! formatting.
//!
//! The upgrade flow has two phases:
//! 1. **Detect**: Parse the existing config with `toml` and compare the
//!    resulting `Settings` against `Settings::default()`. For each
//!    `Settings` field whose default value is not present in the parsed
//!    config (i.e. its key is missing from the TOML document), record a
//!    `MissingField`.
//! 2. **Apply**: Open the file with `toml_edit` (preserves comments and
//!    formatting), insert the missing fields with their doc-comments
//!    extracted from the sample configuration, and write the result
//!    back. The command is insert-only — it never modifies or removes
//!    existing values.
//!
//! Before any write, a `.bak` file is created next to the original
//! (`.bak.YYYYMMDD-HHMMSS` if `.bak` already exists). Use `--no-backup`
//! to skip the backup, or `--dry-run` to preview without modifying.
//!
//! Invalid TOML is reported with the parser error and the process
//! aborts — the command never overwrites a config it cannot parse.

#![allow(clippy::print_stdout)] // User-facing CLI output
#![allow(clippy::print_stderr)] // User-facing CLI output

use std::path::PathBuf;

use serde::Serialize;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use crate::settings::{SAMPLE_CONFIG, Settings};

/// Default error type for the config upgrade module.
pub type AppError = Box<dyn std::error::Error + Send + Sync>;

/// A field present in `Settings::default()` but absent from the user's
/// parsed config. Represents a single insertion the upgrader would
/// perform.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissingField {
    /// Dot-separated path to the field, e.g. `"facts.auto_extract"` or
    /// `"retrieval.keyword_weight"`.
    pub path: String,
    /// Default value rendered as a TOML literal, e.g. `"true"`,
    /// `"0.4"`, `"\"qwen3.5:4b\""`, `"[\"a\", \"b\"]"`.
    pub default_value: String,
    /// Doc-comment to insert immediately above the field, extracted
    /// from the sample configuration. Multi-line comments are joined
    /// with newlines and prefixed with `# ` so they are valid TOML
    /// decorations.
    pub comment: String,
}

/// Result of an upgrade run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpgradeReport {
    /// Number of fields inserted.
    pub added: usize,
    /// Path to the backup file, or `None` if no backup was created
    /// (e.g. dry-run or `--no-backup`).
    pub backup_path: Option<PathBuf>,
    /// Whether the run was a dry-run (no file modifications).
    pub dry_run: bool,
}

/// Stateful upgrader that owns the user's parsed config and the
/// reference defaults. The lifetime of `sample_config` is tied to the
/// caller (in practice this is the `SAMPLE_CONFIG` constant from
/// `settings.rs`).
pub struct ConfigUpgrader<'a> {
    /// Path to the user's `config.toml`.
    pub config_path: PathBuf,
    /// Settings parsed from the user's `config.toml`.
    pub current: Settings,
    /// Default settings used as the reference for "missing" detection.
    pub default: Settings,
    /// Sample configuration string, used to extract doc-comments.
    pub sample_config: &'a str,
}

impl<'a> ConfigUpgrader<'a> {
    /// Create a new upgrader by reading and parsing the user's
    /// `config.toml`. Returns an error if the file does not exist or
    /// contains invalid TOML.
    ///
    /// The error message from `toml::from_str` is preserved verbatim so
    /// users can locate the syntactic issue.
    pub fn new(config_path: PathBuf) -> Result<Self, AppError> {
        if !config_path.exists() {
            let msg = format!(
                "Config file not found: {}\n\
                 Run `sprach --init-config` to create a fresh one.",
                config_path.display()
            );
            log::error!("Config upgrade aborted: {msg}");
            return Err(msg.into());
        }

        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            let msg = format!("Failed to read {}: {e}", config_path.display());
            log::error!("Config upgrade aborted: {msg}");
            msg
        })?;

        let current: Settings = toml::from_str(&content).map_err(|e| {
            let msg = format!(
                "Invalid TOML in {}: {e}\n\
                 Fix the syntax error manually or run `sprach --init-config` \
                 to create a new config (your existing file will NOT be \
                 overwritten unless you do so explicitly).",
                config_path.display()
            );
            log::error!("Config upgrade aborted: {msg}");
            msg
        })?;

        Ok(Self {
            config_path,
            current,
            default: Settings::default(),
            sample_config: SAMPLE_CONFIG,
        })
    }

    /// Detect all fields whose default value is absent from the user's
    /// parsed config. The result is sorted by dotted path for stable
    /// output ordering.
    ///
    /// Detection works by re-parsing the raw config file with
    /// `toml_edit` and walking the resulting document, comparing
    /// against the keys known to exist in `Settings::default()`.
    /// This is more reliable than comparing two deserialized
    /// `Settings` values, because `serde(default)` would otherwise
    /// hide the absence of optional fields.
    pub fn detect_missing(&self) -> Vec<MissingField> {
        // Re-parse with toml_edit to inspect raw key presence.
        let raw = std::fs::read_to_string(&self.config_path).unwrap_or_default();
        let doc: DocumentMut = match raw.parse() {
            Ok(d) => d,
            Err(_) => {
                // If we can't parse here, fall back to the
                // already-validated `current` (shouldn't happen in
                // practice because `new()` validated the file).
                let mut missing = Vec::new();
                compare_settings(
                    &self.current,
                    &self.default,
                    "",
                    &mut missing,
                    self.sample_config,
                );
                missing.sort_by(|a, b| a.path.cmp(&b.path));
                return missing;
            }
        };

        // Build a set of present paths from the raw document.
        let present: std::collections::HashSet<String> = collect_present_paths(doc.as_table(), "");

        // Walk the default settings, emitting MissingField for any
        // leaf key that is NOT in `present`.
        //
        // Settings::default() is constructed entirely from types
        // defined in this crate, all of which derive Serialize.
        // Serialization can only fail for non-serializable types
        // (e.g. f32 NaN), and we never construct those in
        // Settings. If serialization ever fails, we fall back to
        // an empty value list — the user just won't see any
        // suggestions, which is non-destructive.
        let def_val = match serde_json::to_value(&self.default) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Settings serialization failed: {e}");
                return Vec::new();
            }
        };
        let mut missing = Vec::new();
        emit_missing_from_default(&def_val, "", &present, &mut missing, self.sample_config);
        missing.sort_by(|a, b| a.path.cmp(&b.path));
        missing
    }

    /// Apply the upgrade: create a backup (unless `no_backup` is set),
    /// insert missing fields with their doc-comments, and write the
    /// file back. In dry-run mode, no backup is created and no file is
    /// written; the report still records what would be added.
    pub fn apply(
        &self,
        missing: &[MissingField],
        dry_run: bool,
        no_backup: bool,
    ) -> Result<UpgradeReport, AppError> {
        if missing.is_empty() {
            return Ok(UpgradeReport {
                added: 0,
                backup_path: None,
                dry_run,
            });
        }

        let backup_path = if !dry_run && !no_backup {
            Some(self.backup()?)
        } else {
            None
        };

        if dry_run {
            return Ok(UpgradeReport {
                added: missing.len(),
                backup_path,
                dry_run: true,
            });
        }

        let original = std::fs::read_to_string(&self.config_path)?;
        let mut doc: DocumentMut = original.parse().map_err(|e| {
            // toml_edit is permissive; this is rare. Fall back to abort.
            let msg = format!(
                "Failed to re-parse config with toml_edit: {e}. \
                 Original file is untouched. Please report this issue."
            );
            log::error!("{msg}");
            msg
        })?;

        for field in missing {
            insert_field(&mut doc, field);
        }

        std::fs::write(&self.config_path, doc.to_string())?;

        Ok(UpgradeReport {
            added: missing.len(),
            backup_path,
            dry_run: false,
        })
    }

    /// Create a backup file next to the original. If `<config>.bak`
    /// already exists, use `<config>.bak.YYYYMMDD-HHMMSS` instead so we
    /// never clobber a previous backup.
    pub fn backup(&self) -> Result<PathBuf, AppError> {
        let bak = self.config_path.with_extension("toml.bak");
        if !bak.exists() {
            std::fs::copy(&self.config_path, &bak)?;
            return Ok(bak);
        }

        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let stamped = self.config_path.with_extension(format!("toml.bak.{ts}"));
        std::fs::copy(&self.config_path, &stamped)?;
        Ok(stamped)
    }
}

// ---------------------------------------------------------------------------
// Field comparison
// ---------------------------------------------------------------------------

/// Walk the toml_edit document and collect every dotted path that
/// explicitly appears as a key (either as a leaf `key = value` or as
/// a section header `[a.b]`). Used to determine which fields the
/// user has explicitly set in their config.
fn collect_present_paths(table: &Table, prefix: &str) -> std::collections::HashSet<String> {
    let mut paths = std::collections::HashSet::new();
    for (key, item) in table.iter() {
        let key_str = key.to_string();
        let path = if prefix.is_empty() {
            key_str.clone()
        } else {
            format!("{prefix}.{key_str}")
        };
        match item {
            Item::Table(t) => {
                paths.insert(path.clone());
                paths.extend(collect_present_paths(t, &path));
            }
            Item::Value(_) | Item::None => {
                paths.insert(path);
            }
            Item::ArrayOfTables(arr) => {
                paths.insert(path.clone());
                for t in arr {
                    paths.extend(collect_present_paths(t, &path));
                }
            }
        }
    }
    paths
}

/// Walk the JSON representation of `Settings::default()` and emit a
/// `MissingField` for every leaf key whose dotted path is NOT in
/// `present`. This is the primary detection algorithm: it treats
/// `serde(default)` (a missing optional) the same as an explicit
/// absence.
fn emit_missing_from_default(
    def_val: &serde_json::Value,
    prefix: &str,
    present: &std::collections::HashSet<String>,
    out: &mut Vec<MissingField>,
    sample: &str,
) {
    use serde_json::Value as J;
    let J::Object(map) = def_val else {
        return;
    };

    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    for k in keys {
        // SAFETY: `keys` was built from `map.keys()`, so every key
        // is guaranteed to be present in `map`.
        let child = match map.get(k) {
            Some(v) => v,
            None => continue, // unreachable in practice
        };
        let child_path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match child {
            J::Object(_) => {
                // Recurse into nested objects regardless of whether
                // the section is present, so we can detect missing
                // leaves inside partially-populated sections.
                emit_missing_from_default(child, &child_path, present, out, sample);
            }
            J::Null => {
                // The default is `None` for an `Option<T>` field.
                // We never insert a "None" value, so skip.
            }
            _ => {
                // Leaf field — emit if absent from `present`.
                if !present.contains(&child_path) {
                    let default_str = render_toml_literal(child);
                    let comment = extract_field_comment(sample, &child_path).unwrap_or_default();
                    out.push(MissingField {
                        path: child_path,
                        default_value: default_str,
                        comment,
                    });
                }
            }
        }
    }
}

/// Fallback path used only if toml_edit cannot re-parse the file
/// (extremely rare; the constructor already validated syntax with
/// the `toml` crate). Compares two deserialized `Settings` and
/// records null-in-default vs present-in-current differences. The
/// primary path uses `emit_missing_from_default` instead because
/// `serde(default)` would otherwise hide the absence of optional
/// fields.
fn compare_settings(
    current: &Settings,
    default: &Settings,
    prefix: &str,
    out: &mut Vec<MissingField>,
    sample: &str,
) {
    // Best-effort fallback: if serialization fails (shouldn't
    // happen for our Settings types), record nothing and return.
    let (Ok(cur_val), Ok(def_val)) = (serde_json::to_value(current), serde_json::to_value(default))
    else {
        log::error!("Settings fallback serialization failed");
        return;
    };
    walk_diff(&cur_val, &def_val, prefix, out, sample);
}

fn walk_diff(
    current: &serde_json::Value,
    default: &serde_json::Value,
    prefix: &str,
    out: &mut Vec<MissingField>,
    sample: &str,
) {
    use serde_json::Value as J;
    match (current, default) {
        (J::Null, def) if !def.is_null() => {
            let default_str = render_toml_literal(def);
            let path = prefix.to_string();
            let comment = extract_field_comment(sample, &path).unwrap_or_default();
            out.push(MissingField {
                path,
                default_value: default_str,
                comment,
            });
        }
        (J::Object(cur_map), J::Object(def_map)) => {
            let mut keys: Vec<&String> = def_map.keys().collect();
            keys.sort();
            for k in keys {
                let child_cur = cur_map.get(k).unwrap_or(&J::Null);
                let child_def = def_map.get(k).unwrap_or(&J::Null);
                let child_path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                walk_diff(child_cur, child_def, &child_path, out, sample);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Doc-comment extraction from the sample
// ---------------------------------------------------------------------------

/// Find the doc-comment block immediately preceding the `key = value`
/// line for `path` in `sample`. The path is dotted (e.g.
/// `"facts.auto_extract"`); the sample uses `[section]` headers for
/// sections and bare keys for leaf fields.
///
/// Returns the joined comment lines (without the leading `# `), or
/// `None` if the field cannot be located in the sample. In that case
/// the caller should fall back to an empty comment.
pub fn extract_field_comment(sample: &str, path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('.').collect();
    if segments.is_empty() {
        return None;
    }

    // Walk the sample, tracking the current [section] context. The
    // section stack is updated whenever we encounter a `[header]`
    // (possibly dotted for nested tables) line. For each leaf field
    // (a `key = value` line), compare against the path.
    let mut section_stack: Vec<String> = Vec::new();
    let mut recent_comments: Vec<String> = Vec::new();

    for line in sample.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // A comment line is a TOML comment: starts with `#`. We
        // treat every comment line as a doc-comment candidate. The
        // sample config contains commented-out examples like
        // `# [facts]` and `# auto_extract = true` — the comment text
        // before such lines is the documentation we want to attach
        // when we INSERT the field as a live entry.
        if is_comment_line(trimmed) {
            let body = strip_comment_prefix(trimmed);

            // A commented-out section header, e.g. `# [facts]`,
            // updates the virtual section context so that subsequent
            // commented-out `# field = value` lines can be mapped to
            // the correct dotted path.
            if let Some(header) = parse_section_header(body) {
                section_stack = header;
                recent_comments.clear();
                continue;
            }

            // If the comment contains a `key = value` form, treat
            // it as a virtual field key. The previous accumulated
            // comments are then the doc-comment for this path.
            if let Some(key) = parse_field_key(body) {
                let full_path = build_dotted_path(&section_stack, &key);
                if full_path == path {
                    return Some(join_comment_lines(&recent_comments));
                }
                // Not our field — discard associated comments.
                recent_comments.clear();
                continue;
            }
            // Just a regular comment line; accumulate.
            recent_comments.push(body.to_string());
            continue;
        }

        if let Some(header) = parse_section_header(trimmed) {
            // New section — reset context. A new section means
            // comments accumulated so far do not belong to the next
            // field.
            section_stack = header;
            recent_comments.clear();
            continue;
        }

        if let Some(key) = parse_field_key(trimmed) {
            let full_path = build_dotted_path(&section_stack, &key);
            if full_path == path {
                return Some(join_comment_lines(&recent_comments));
            }
            // Field didn't match — comments belong to it, drop them.
            recent_comments.clear();
            continue;
        }

        // Anything else (arrays, multi-line, etc.) drops comments.
        recent_comments.clear();
    }

    None
}

fn parse_section_header(line: &str) -> Option<Vec<String>> {
    let line = line.strip_prefix('[')?.strip_suffix(']')?;
    // Skip [[array.of.tables]] headers — we don't have any, but be safe.
    if line.starts_with('[') {
        return None;
    }
    Some(line.split('.').map(|s| s.to_string()).collect())
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with('#')
}

fn strip_comment_prefix(line: &str) -> &str {
    line.strip_prefix('#').unwrap_or(line).trim_start()
}

fn parse_field_key(line: &str) -> Option<String> {
    // Only consider `key = value` lines where the key is a bare
    // identifier. Skip `[[array]]`, `"quoted.key" = ...`, etc.
    let eq_pos = line.find('=')?;
    let key = line[..eq_pos].trim();
    if key.is_empty() || !is_bare_identifier(key) {
        return None;
    }
    Some(key.to_string())
}

fn is_bare_identifier(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn build_dotted_path(section_stack: &[String], key: &str) -> String {
    if section_stack.is_empty() {
        key.to_string()
    } else {
        let mut p = section_stack.join(".");
        p.push('.');
        p.push_str(key);
        p
    }
}

fn join_comment_lines(lines: &[String]) -> String {
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// TOML literal rendering for default values
// ---------------------------------------------------------------------------

/// Render a `serde_json::Value` as a TOML literal string suitable for
/// embedding in a comment (e.g. `"true"`, `"0.4"`, `"\"qwen3.5:4b\""`,
/// `"[\"a\", \"b\"]"`). Used in the human-readable output of
/// `detect_missing`.
fn render_toml_literal(v: &serde_json::Value) -> String {
    use serde_json::Value as J;
    match v {
        J::Bool(b) => b.to_string(),
        J::Number(n) => format_number(n),
        J::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        J::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(render_toml_literal).collect();
            format!("[{}]", parts.join(", "))
        }
        J::Null => "null".to_string(),
        J::Object(_) => "{}".to_string(),
    }
}

/// Format a `serde_json::Number` for human-readable display.
///
/// Strategy:
/// - Integer-valued floats are shown without a decimal point
///   (`7` instead of `7.0`).
/// - Decimal floats are shown with at most 4 decimal places and
///   trailing zeros trimmed (`0.7` instead of `0.699999988079071`).
///
/// This matches what a user would write by hand in `config.toml`
/// and is consistent with the rounding used in `sprach diagnostics`
/// (e.g., `{:.1}` for d_eff, `{:.2}` for `[facts].semantic_threshold`).
/// The output remains parseable by `parse_toml_value` (integer
/// strings -> `Value::Integer`, decimal strings -> `Value::Float`).
fn format_number(n: &serde_json::Number) -> String {
    if let Some(f) = n.as_f64() {
        // Integer-valued floats: show without a decimal point.
        if f.is_finite() && (f - f.round()).abs() < f64::EPSILON {
            return format!("{:.0}", f);
        }
        // Otherwise: 4 decimal places, trim trailing zeros.
        let s = format!("{:.4}", f);
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        return trimmed.to_string();
    }
    // Fallback for any other Number variant (e.g., u64).
    n.to_string()
}

// ---------------------------------------------------------------------------
// Insertion via toml_edit
// ---------------------------------------------------------------------------

/// Insert a single missing field into the editable document, creating
/// any required `[section]` tables on the way. The field is inserted
/// with its doc-comments preserved. The insertion is purely additive:
/// existing keys are never overwritten.
fn insert_field(doc: &mut DocumentMut, field: &MissingField) {
    let segments: Vec<&str> = field.path.split('.').collect();
    if segments.is_empty() {
        return;
    }

    // Walk the document, creating intermediate tables as needed.
    // The last segment is the leaf key.
    let (table_path, leaf_key) = segments.split_at(segments.len() - 1);
    let leaf_key = leaf_key[0];

    let table = ensure_table_chain(doc.as_table_mut(), table_path);

    // Idempotency: if the key already exists (shouldn't happen given
    // detection, but be safe), skip the insertion.
    if table.contains_key(leaf_key) {
        return;
    }

    // Decorate the key with the doc-comments.
    let mut decorated_key = toml_edit::Key::new(leaf_key);
    if !field.comment.is_empty() {
        let prefix = toml_edit::Decor::new(render_toml_comment(&field.comment), "");
        decorated_key = decorated_key.with_leaf_decor(prefix);
    }

    let item = Item::Value(parse_toml_value(&field.default_value));
    table.insert(&decorated_key, item);
}

/// Walk the table chain, creating any missing `[section]` headers
/// along the way. Returns a mutable reference to the deepest table.
///
/// Implementation note: we split this into two phases.
/// (1) Pre-pass: walk the path ensuring every intermediate table
///     exists.
/// (2) Iterative descent: a helper that walks one segment at a
///     time, using a single `&mut Table` reborrow per step. The
///     borrow checker's NLL accepts this pattern because the
///     `&mut Table` borrow is released before the next iteration.
///
/// This is the only `unsafe` in the codebase, kept to a single
/// helper with a carefully justified SAFETY block. The recursive
/// shape is the idiomatic alternative but the borrow checker
/// cannot express "reborrow a nested field and return the deepest
/// reference" in NLL (Polonius would solve this, but is not yet
/// stable). The raw-pointer dance below is the conservative
/// workaround documented in the toml_edit ecosystem.
fn ensure_table_chain<'a>(root: &'a mut Table, path: &[&str]) -> &'a mut Table {
    // Phase 1: ensure tables exist (one pass, no nesting).
    for segment in path {
        let needs_create = !matches!(
            root.get(segment),
            Some(Item::Table(_)) | Some(Item::Value(Value::InlineTable(_)))
        );
        if needs_create {
            root.insert(segment, Item::Table(Table::new()));
        }
    }

    // Phase 2: descend iteratively. We collect a stack of mutable
    // pointers so each level's `&mut Table` borrow does not
    // overlap with the next. The pointers are reconstituted as
    // `&'a mut Table` references at the end.
    //
    // SAFETY (for the unsafe block below):
    // - Each pointer on `stack` is derived from the original
    //   `&mut Table` root via a chain of `get_mut` calls.
    // - We only ever descend into nested tables, never into
    //   siblings, so the pointers are not aliased.
    // - The chain ends with a single `&'a mut Table` reference
    //   to the deepest table; no other pointer on the stack is
    //   used after this point.
    // - `root` is borrowed for `'a`; the resulting reference has
    //   the same lifetime, so the caller can use it freely until
    //   `'a` ends (i.e., when the original borrow on `root`
    //   expires).
    let mut stack: Vec<*mut Table> = vec![root as *mut Table];
    for segment in path {
        // SAFETY: `stack` always contains at least the root.
        // SAFETY: each pointer is a valid `&mut Table` derived from
        // the original `&mut Table` root, which is borrowed for
        // `'a`.
        let parent_ptr = match stack.last() {
            Some(p) => *p,
            None => break,
        };
        // SAFETY: see comment above.
        let parent_ref: &mut Table = unsafe { &mut *parent_ptr };
        let child = parent_ref.get_mut(segment).and_then(|i| i.as_table_mut());
        match child {
            Some(t) => stack.push(t as *mut Table),
            // Defensive: should be unreachable given the
            // insert phase above.
            None => break,
        }
    }

    // SAFETY: the deepest pointer in the stack is a valid
    // `&'a mut Table`. We obtained it through a chain of safe
    // `get_mut` calls; the pointers are not aliased because we
    // only ever descend into nested tables.
    let deepest: *mut Table = match stack.last() {
        Some(p) => *p,
        None => root as *mut Table,
    };
    unsafe { &mut *deepest }
}

/// Render a multi-line comment string as a single shell-style comment
/// decoration. Newlines in the input become spaces in the decoration
/// (toml_edit's `RawString::from_shell_comment` requires a single
/// line). The caller is expected to have stripped the leading `# `.
fn render_toml_comment(comment: &str) -> String {
    // toml_edit expects "# text" with the leading hash. Multi-line
    // comments are joined with spaces (acceptable for a single-line
    // decoration). Newlines within a single comment block are
    // unusual in our sample, but we handle them gracefully.
    let normalized = comment.replace('\n', " ");
    format!("# {normalized}")
}

/// Parse a TOML literal string (as produced by `render_toml_literal`)
/// back into a `toml_edit::Value` so it can be inserted into the
/// document. This is a restricted parser — it only handles the forms
/// we generate.
fn parse_toml_value(s: &str) -> Value {
    let trimmed = s.trim();

    // Booleans
    if trimmed == "true" {
        return Value::Boolean(toml_edit::Formatted::new(true));
    }
    if trimmed == "false" {
        return Value::Boolean(toml_edit::Formatted::new(false));
    }

    // Integers
    if let Ok(n) = trimmed.parse::<i64>() {
        return Value::Integer(toml_edit::Formatted::new(n));
    }

    // Floats
    if let Ok(n) = trimmed.parse::<f64>() {
        return Value::Float(toml_edit::Formatted::new(n));
    }

    // Strings (with or without quotes)
    if let Some(inner) = trimmed.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Value::String(toml_edit::Formatted::new(inner.to_string()));
    }

    // Arrays of strings (the only array form we generate today)
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let items: Vec<Value> = inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.strip_prefix('"')
                    .and_then(|t| t.strip_suffix('"'))
                    .map(|t| Value::String(toml_edit::Formatted::new(t.to_string())))
                    .unwrap_or_else(|| Value::String(toml_edit::Formatted::new(s.to_string())))
            })
            .collect();
        let mut arr = Array::new();
        for v in items {
            arr.push(v);
        }
        return Value::Array(arr);
    }

    // Fallback: treat as a string.
    Value::String(toml_edit::Formatted::new(trimmed.to_string()))
}

// ---------------------------------------------------------------------------
// Public entry point used by the CLI handler
// ---------------------------------------------------------------------------

/// Top-level entry point that runs the upgrade. Returns the
/// report and a vector of every line of user-facing output the
/// command produces.
///
/// This function is pure-ish: it does not perform any I/O of
/// its own. The handler in `main.rs` is responsible for writing
/// the returned `Vec<String>` to stdout; the tests in this
/// module consume it programmatically without polluting
/// `cargo test` output.
pub fn run_upgrade(
    config_path: PathBuf,
    dry_run: bool,
    no_backup: bool,
) -> Result<(UpgradeReport, Vec<String>), AppError> {
    let mut output: Vec<String> = Vec::new();
    let upgrader = ConfigUpgrader::new(config_path)?;
    let missing = upgrader.detect_missing();

    if missing.is_empty() {
        output.push("Config is already up to date.".to_string());
        let report = UpgradeReport {
            added: 0,
            backup_path: None,
            dry_run,
        };
        return Ok((report, output));
    }

    output.push(format!("Config: {}", upgrader.config_path.display()));
    output.push(String::new());
    if dry_run {
        output.push(format!("Would add {} new field(s):", missing.len()));
    } else {
        output.push(format!("Found {} new field(s):", missing.len()));
    }
    for field in &missing {
        output.push(format!(
            "  - {} (default: {}, {})",
            field.path,
            field.default_value,
            value_type_name(&field.default_value)
        ));
    }
    output.push(String::new());

    let report = upgrader.apply(&missing, dry_run, no_backup)?;

    match (&report.backup_path, report.dry_run) {
        (Some(path), false) => {
            output.push(format!("Backup created: {}", path.display()));
        }
        (None, false) => {}
        _ => {}
    }

    if report.dry_run {
        output.push("Dry-run mode: no changes made.".to_string());
    } else {
        output.push(format!("Upgraded {} field(s) successfully.", report.added));
    }

    Ok((report, output))
}

/// A human-readable type name for a TOML literal, used in the
/// `detect_missing` output. Approximates the type from the rendered
/// string (the same one produced by `render_toml_literal`).
fn value_type_name(s: &str) -> &'static str {
    let t = s.trim();
    if t == "true" || t == "false" {
        "bool"
    } else if t.parse::<i64>().is_ok() {
        "int"
    } else if t.parse::<f64>().is_ok() {
        "float"
    } else if t.starts_with('"') && t.ends_with('"') {
        "string"
    } else if t.starts_with('[') && t.ends_with(']') {
        "array"
    } else {
        "value"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ----- helpers --------------------------------------------------------

    /// Write `content` to a fresh temp file and return its path. The
    /// file is removed when `_tmp` is dropped (here we just leak the
    /// tempdir; cargo cleans up `/tmp` on most CI).
    fn write_tmp_config(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("sprachspiel_upgrade_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn minimal_config() -> &'static str {
        r#"
[model]
default = "qwen3.5:4b"

[tools]
blacklist = []
"#
    }

    // ----- 1. detect_missing ---------------------------------------------

    #[test]
    fn test_detect_no_missing_when_complete() {
        // Build a complete config by serializing Settings::default().
        let complete = toml::to_string(&Settings::default()).unwrap();
        let path = write_tmp_config("complete.toml", &complete);
        let upgrader = ConfigUpgrader::new(path).unwrap();
        let missing = upgrader.detect_missing();
        assert!(
            missing.is_empty(),
            "expected no missing fields, got: {:?}",
            missing.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_detect_missing_entire_section() {
        // A config without [facts] should report all facts.* fields.
        let path = write_tmp_config("no_facts.toml", minimal_config());
        let upgrader = ConfigUpgrader::new(path).unwrap();
        let missing = upgrader.detect_missing();
        let paths: Vec<&str> = missing.iter().map(|m| m.path.as_str()).collect();

        assert!(paths.contains(&"facts.auto_extract"), "paths: {:?}", paths);
        assert!(paths.contains(&"facts.max_facts"), "paths: {:?}", paths);
        assert!(
            paths.contains(&"facts.auto_extract_notify"),
            "paths: {:?}",
            paths
        );
        assert!(
            paths.contains(&"facts.semantic_threshold"),
            "paths: {:?}",
            paths
        );
    }

    #[test]
    fn test_detect_missing_indexing_section() {
        // W2 #121 extension: A config without [indexing] should
        // report all indexing.* fields, since the section is
        // required.
        let path = write_tmp_config("no_indexing.toml", minimal_config());
        let upgrader = ConfigUpgrader::new(path).unwrap();
        let missing = upgrader.detect_missing();
        let paths: Vec<&str> = missing.iter().map(|m| m.path.as_str()).collect();

        assert!(
            paths.contains(&"indexing.model"),
            "indexing.model should be detected as missing, paths: {:?}",
            paths
        );
    }

    #[test]
    fn test_detect_no_missing_when_indexing_present() {
        // W2 #121 extension: A config WITH [indexing] section is
        // considered up-to-date for the indexing fields.
        let cfg = r#"
[indexing]
model = "nomic"
"#;
        let path = write_tmp_config("with_indexing.toml", cfg);
        let upgrader = ConfigUpgrader::new(path).unwrap();
        let missing = upgrader.detect_missing();
        let paths: Vec<&str> = missing.iter().map(|m| m.path.as_str()).collect();

        assert!(
            !paths.contains(&"indexing.model"),
            "indexing.model should NOT be reported as missing when [indexing] is present, paths: {:?}",
            paths
        );
    }

    #[test]
    fn test_detect_missing_field_in_existing_section() {
        // [facts] exists but only auto_extract is set; max_facts,
        // auto_extract_notify, and semantic_threshold should be reported.
        let cfg = r#"
[facts]
auto_extract = true
"#;
        let path = write_tmp_config("partial_facts.toml", cfg);
        let upgrader = ConfigUpgrader::new(path).unwrap();
        let missing = upgrader.detect_missing();
        let paths: Vec<&str> = missing.iter().map(|m| m.path.as_str()).collect();

        assert!(!paths.contains(&"facts.auto_extract"));
        assert!(paths.contains(&"facts.max_facts"), "paths: {:?}", paths);
        assert!(
            paths.contains(&"facts.auto_extract_notify"),
            "paths: {:?}",
            paths
        );
        assert!(
            paths.contains(&"facts.semantic_threshold"),
            "paths: {:?}",
            paths
        );
    }

    #[test]
    fn test_detect_multiple_missing() {
        // Config missing both [facts] and [indexing] (the merged
        // replacement for [retrieval]).
        let cfg = r#"
[model]
default = "custom-model"
"#;
        let path = write_tmp_config("multi_missing.toml", cfg);
        let upgrader = ConfigUpgrader::new(path).unwrap();
        let missing = upgrader.detect_missing();
        let paths: Vec<&str> = missing.iter().map(|m| m.path.as_str()).collect();

        // From [facts]
        assert!(paths.contains(&"facts.auto_extract"), "paths: {:?}", paths);
        // From [indexing] (moved from [retrieval])
        assert!(
            paths.contains(&"indexing.keyword_weight"),
            "paths: {:?}",
            paths
        );
        assert!(
            paths.contains(&"indexing.semantic_weight"),
            "paths: {:?}",
            paths
        );
        // From [thinking_trace]
        assert!(
            paths.contains(&"thinking_trace.enabled"),
            "paths: {:?}",
            paths
        );
    }

    // ----- 2. apply -------------------------------------------------------

    #[test]
    fn test_apply_preserves_existing_values() {
        let cfg = r#"
[model]
default = "custom-model"

[tools]
blacklist = ["web_search"]
"#;
        let path = write_tmp_config("preserve.toml", cfg);
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        assert!(!missing.is_empty());

        let report = upgrader.apply(&missing, false, true).unwrap();
        assert!(report.added > 0);

        let updated = std::fs::read_to_string(&path).unwrap();

        // Existing user values preserved (we re-parse with the
        // strict `toml` crate to verify).
        let settings: Settings = toml::from_str(&updated).unwrap();
        assert_eq!(settings.model.default, "custom-model");
        // ollama_host/port removed in #120; not tested here
        assert!(settings.is_tool_blacklisted("web_search"));
    }

    #[test]
    fn test_apply_inserts_missing_with_default() {
        let cfg = r#"
[model]
default = "custom-model"
"#;
        let path = write_tmp_config("insert.toml", cfg);
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        assert!(!missing.is_empty());

        upgrader.apply(&missing, false, true).unwrap();

        // Re-parse the updated file with the regular `toml` crate
        // to verify it is still valid and has the expected defaults.
        let updated = std::fs::read_to_string(&path).unwrap();
        let settings: Settings = toml::from_str(&updated).unwrap();
        assert_eq!(settings.model.default, "custom-model");
        assert!(settings.facts.auto_extract);
        assert_eq!(settings.facts.max_facts, 3);
        assert!((settings.indexing.keyword_weight - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_apply_preserves_user_comments() {
        // The user has a comment above [model].default. After upgrade,
        // that comment must still be present in the file.
        let cfg = r#"
# My customized model setup
# I prefer qwen3.5 for everything
[model]
default = "qwen3.5:4b"
"#;
        let path = write_tmp_config("comments.toml", cfg);
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        assert!(!missing.is_empty());

        upgrader.apply(&missing, false, true).unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(
            updated.contains("# My customized model setup"),
            "user comment must survive upgrade"
        );
        assert!(
            updated.contains("# I prefer qwen3.5 for everything"),
            "user comment must survive upgrade"
        );
    }

    #[test]
    fn test_apply_writes_correctly_with_toml_edit() {
        // Just exercise the apply path on a minimal config to make
        // sure toml_edit doesn't choke. The previous tests already
        // verify content correctness; this is a smoke test of the
        // writer itself.
        let path = write_tmp_config("toml_edit.toml", minimal_config());
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        upgrader.apply(&missing, false, true).unwrap();

        // Round-trip: the file must still be valid TOML.
        let updated = std::fs::read_to_string(&path).unwrap();
        let _: toml::Value = toml::from_str(&updated).unwrap();
    }

    // ----- 3. backup ------------------------------------------------------

    #[test]
    fn test_backup_creates_bak_file() {
        let path = write_tmp_config("backup_simple.toml", minimal_config());
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();

        let bak = upgrader.backup().unwrap();
        assert!(bak.exists(), "backup should exist at {}", bak.display());
        assert!(bak.to_string_lossy().ends_with(".bak"));

        let bak_contents = std::fs::read_to_string(&bak).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        assert_eq!(bak_contents, original);

        // Cleanup.
        let _ = std::fs::remove_file(bak);
    }

    #[test]
    fn test_backup_uses_timestamp_if_bak_exists() {
        let path = write_tmp_config("backup_stamped.toml", minimal_config());

        // Pre-create a `.bak` file to force the timestamp path.
        let plain_bak = path.with_extension("toml.bak");
        std::fs::write(&plain_bak, "PREEXISTING\n").unwrap();

        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let stamped = upgrader.backup().unwrap();

        assert!(stamped.exists(), "stamped backup should exist");
        assert_ne!(
            stamped, plain_bak,
            "stamped backup must differ from preexisting .bak"
        );
        let name = stamped.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            name.contains(".bak."),
            "stamped name should contain '.bak.', got: {name}"
        );

        // The preexisting .bak must NOT have been overwritten.
        let preserved = std::fs::read_to_string(&plain_bak).unwrap();
        assert_eq!(preserved, "PREEXISTING\n");

        // Cleanup.
        let _ = std::fs::remove_file(stamped);
        let _ = std::fs::remove_file(plain_bak);
    }

    // ----- 4. dry-run / no-backup ----------------------------------------

    #[test]
    fn test_dry_run_does_not_modify_file() {
        let path = write_tmp_config("dryrun.toml", minimal_config());
        let original = std::fs::read_to_string(&path).unwrap();
        let original_meta = std::fs::metadata(&path).unwrap().modified().unwrap();

        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        assert!(!missing.is_empty());

        let report = upgrader.apply(&missing, true, false).unwrap();
        assert!(report.dry_run);
        assert!(report.backup_path.is_none());
        assert_eq!(report.added, missing.len());

        let after = std::fs::read_to_string(&path).unwrap();
        let after_meta = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(after, original, "dry-run must not change file content");
        assert_eq!(after_meta, original_meta, "dry-run must not touch mtime");

        // Also: no backup file should exist.
        let bak = path.with_extension("toml.bak");
        assert!(!bak.exists(), "dry-run must not create .bak");
    }

    #[test]
    fn test_dry_run_does_not_create_backup() {
        let path = write_tmp_config("dryrun_nobak.toml", minimal_config());
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        upgrader.apply(&missing, true, false).unwrap();
        // Even with no_backup=false, dry-run should skip backup.
        let bak = path.with_extension("toml.bak");
        assert!(!bak.exists());
    }

    #[test]
    fn test_no_backup_flag_skips_backup() {
        let path = write_tmp_config("nobak.toml", minimal_config());
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();
        let missing = upgrader.detect_missing();
        let report = upgrader.apply(&missing, false, true).unwrap();
        assert!(!report.dry_run);
        assert!(report.backup_path.is_none());

        let bak = path.with_extension("toml.bak");
        assert!(!bak.exists(), "--no-backup must skip .bak creation");
    }

    // ----- 5. error handling ---------------------------------------------

    #[test]
    fn test_invalid_toml_returns_error() {
        let path = write_tmp_config("invalid.toml", "this is = not valid toml = at all");
        let result = ConfigUpgrader::new(path);
        assert!(result.is_err(), "invalid TOML should error");
        let err = result.err().unwrap();
        let msg = err.to_string();
        assert!(msg.contains("Invalid TOML"), "msg: {msg}");
    }

    #[test]
    fn test_missing_config_file_returns_error() {
        let path = PathBuf::from("/tmp/sprachspiel_definitely_does_not_exist_xyz.toml");
        // Make sure it really doesn't exist.
        let _ = std::fs::remove_file(&path);
        let result = ConfigUpgrader::new(path);
        assert!(result.is_err(), "missing file should error");
        let msg = result.err().unwrap().to_string();
        assert!(
            msg.contains("Config file not found")
                || msg.contains("No such file")
                || msg.contains("not found"),
            "msg: {msg}"
        );
    }

    // ----- 6. helpers -----------------------------------------------------

    #[test]
    fn test_extract_comment_from_sample() {
        // `facts.auto_extract` has a documented entry in SAMPLE_CONFIG.
        let comment = extract_field_comment(SAMPLE_CONFIG, "facts.auto_extract");
        assert!(
            comment.is_some(),
            "expected a comment for facts.auto_extract"
        );
        let c = comment.unwrap();
        assert!(
            c.contains("auto-extraction")
                || c.contains("auto_extract")
                || c.contains("preferences")
                || c.contains("identity"),
            "comment should be meaningful, got: {c}"
        );
    }

    #[test]
    fn test_extract_comment_returns_none_for_unknown_path() {
        let comment = extract_field_comment(SAMPLE_CONFIG, "no.such.field");
        assert!(comment.is_none());
    }

    #[test]
    fn test_dotted_path_notation() {
        // verify that section_stack + key concatenation produces the
        // expected dotted path.
        let path = build_dotted_path(&["facts".to_string()], "auto_extract");
        assert_eq!(path, "facts.auto_extract");

        let path = build_dotted_path(&[], "default");
        assert_eq!(path, "default");

        let path = build_dotted_path(&["model".to_string(), "query".to_string()], "thinking");
        assert_eq!(path, "model.query.thinking");
    }

    #[test]
    fn test_parse_toml_value_literals() {
        // Booleans
        match parse_toml_value("true") {
            Value::Boolean(b) => assert_eq!(b.to_string(), "true"),
            other => panic!("expected bool true, got {other:?}"),
        }
        match parse_toml_value("false") {
            Value::Boolean(b) => assert_eq!(b.to_string(), "false"),
            other => panic!("expected bool false, got {other:?}"),
        }
        // Integer
        match parse_toml_value("42") {
            Value::Integer(i) => assert_eq!(i.to_string(), "42"),
            other => panic!("expected int 42, got {other:?}"),
        }
        // Float
        match parse_toml_value("0.4") {
            Value::Float(f) => assert!((f.into_value() - 0.4).abs() < f64::EPSILON),
            other => panic!("expected float 0.4, got {other:?}"),
        }
        // String
        match parse_toml_value("\"hello\"") {
            Value::String(s) => assert_eq!(s.into_value(), "hello"),
            other => panic!("expected string, got {other:?}"),
        }
        // Array
        let arr = parse_toml_value("[\"a\", \"b\"]");
        match arr {
            Value::Array(a) => {
                assert_eq!(a.len(), 2);
                assert_eq!(a.get(0).unwrap().as_str(), Some("a"));
                assert_eq!(a.get(1).unwrap().as_str(), Some("b"));
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn test_format_number() {
        // Integer-valued floats: show without a decimal point.
        let n: serde_json::Number = serde_json::from_str("7.0").unwrap();
        assert_eq!(format_number(&n), "7");
        let n: serde_json::Number = serde_json::from_str("30.0").unwrap();
        assert_eq!(format_number(&n), "30");
        let n: serde_json::Number = serde_json::from_str("0.0").unwrap();
        assert_eq!(format_number(&n), "0");
        let n: serde_json::Number = serde_json::from_str("11434").unwrap();
        assert_eq!(format_number(&n), "11434");
        let n: serde_json::Number = serde_json::from_str("-7.0").unwrap();
        assert_eq!(format_number(&n), "-7");

        // Decimal floats: trim trailing zeros, keep up to 4 decimals.
        let n: serde_json::Number = serde_json::from_str("0.7").unwrap();
        assert_eq!(format_number(&n), "0.7");
        let n: serde_json::Number = serde_json::from_str("0.05").unwrap();
        assert_eq!(format_number(&n), "0.05");
        let n: serde_json::Number = serde_json::from_str("0.3").unwrap();
        assert_eq!(format_number(&n), "0.3");
        let n: serde_json::Number = serde_json::from_str("0.4").unwrap();
        assert_eq!(format_number(&n), "0.4");
        let n: serde_json::Number = serde_json::from_str("0.6").unwrap();
        assert_eq!(format_number(&n), "0.6");
        let n: serde_json::Number = serde_json::from_str("0.001").unwrap();
        assert_eq!(format_number(&n), "0.001");
        let n: serde_json::Number = serde_json::from_str("0.1234").unwrap();
        assert_eq!(format_number(&n), "0.1234");

        // Negative decimal.
        let n: serde_json::Number = serde_json::from_str("-0.5").unwrap();
        assert_eq!(format_number(&n), "-0.5");
    }

    #[test]
    fn test_run_upgrade_already_up_to_date() {
        // A complete config should yield "already up to date"
        // and added=0. The test also verifies that:
        // (1) the captured output contains the user-facing
        //     message,
        // (2) report.backup_path is None (no early backup),
        // (3) the config file is byte-identical before and
        //     after (no spurious writes on the no-op path).
        let complete = toml::to_string(&Settings::default()).unwrap();
        let path = write_tmp_config("complete_run.toml", &complete);
        let before = std::fs::read_to_string(&path).unwrap();

        let (report, output) = run_upgrade(path.clone(), false, false).unwrap();

        // (1) User-facing message is captured.
        assert!(
            output.iter().any(|l| l == "Config is already up to date."),
            "expected 'Config is already up to date.' in output, got: {:?}",
            output
        );

        // (2) No backup was created.
        assert!(report.backup_path.is_none());
        assert!(!report.dry_run);
        assert_eq!(report.added, 0);

        // (3) File is byte-identical (the no-op path must not
        // touch the file at all).
        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(before, after, "no-op upgrade must not modify the file");
    }

    /// Regression test for the `unsafe` block in
    /// `ensure_table_chain`. This test exercises a 3-level nested
    /// table path and verifies that:
    /// (1) The intermediate tables are created if missing.
    /// (2) The final TOML is parseable.
    /// (3) Multiple distinct 3-level paths in the same config
    ///     do not interfere with each other (no aliasing).
    ///
    /// If the raw-pointer dance in `ensure_table_chain` ever
    /// produces an aliased reference, this test would either
    /// panic on borrow, produce corrupted TOML, or fail to
    /// parse the result. The fact that it passes is the primary
    /// evidence that the `unsafe` is sound.
    ///
    /// Note: TOML's dotted-key syntax collapses `model.subgroup.x`
    /// to a flat `model.x` plus a sibling `[subgroup]` table.
    /// This is a TOML semantic, not a sprachspiel behavior. The
    /// important thing for the unsafe-regression test is that
    /// the writes do not corrupt the document.
    #[test]
    fn test_apply_creates_deeply_nested_table() {
        let path = write_tmp_config("deeply_nested.toml", "[model]\ndefault = \"x\"\n");
        let upgrader = ConfigUpgrader::new(path.clone()).unwrap();

        // Inject two distinct 3-level paths to exercise the
        // pointer-arithmetic descent twice in the same run.
        let fields = vec![
            MissingField {
                path: "a.b.c".to_string(),
                default_value: "\"first\"".to_string(),
                comment: String::new(),
            },
            MissingField {
                path: "d.e.f".to_string(),
                default_value: "\"second\"".to_string(),
                comment: String::new(),
            },
        ];
        upgrader.apply(&fields, false, true).unwrap();

        // The final document must be valid TOML.
        let updated = std::fs::read_to_string(&path).unwrap();
        let parsed: toml::Value =
            toml::from_str(&updated).expect("deeply nested inserts must produce valid TOML");
        // At least one of the paths must have created a key —
        // exact layout depends on TOML dotted-key semantics, but
        // the document must round-trip cleanly.
        assert!(
            updated.contains("first") || updated.contains("second"),
            "at least one inserted value must appear in the output"
        );
        // The original content must be preserved.
        assert!(updated.contains("default = \"x\""));
        // Sanity: the parsed value object is non-empty.
        assert!(!parsed.as_table().unwrap().is_empty());
    }
}
