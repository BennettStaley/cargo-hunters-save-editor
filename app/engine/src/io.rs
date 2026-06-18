//! Low-level save file I/O: load, byte-faithful serialize, backup, atomic write.
//!
//! The serialized output is engineered to match the Python engine's
//! `json.dumps(data, indent=4, ensure_ascii=False)` byte-for-byte:
//!   * `serde_json` is built with `arbitrary_precision` so every number is kept
//!     as its original literal and re-emitted verbatim (no float reformatting,
//!     no >2^53 integer loss);
//!   * `preserve_order` keeps object key order;
//!   * a 4-space `PrettyFormatter` matches Python's indent/`": "`/`,` style;
//!   * no trailing newline, LF line endings.
//!
//! The oracle harness (`bin/oracle.rs` + the Python side) proves this match on
//! real saves before any mutation is trusted.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// A loaded save is just a JSON value; typed access happens in `model`/`ops`.
pub type Save = Value;

#[derive(Debug)]
pub enum IoError {
    Read(String),
    Parse(String),
    Write(String),
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::Read(m) => write!(f, "read error: {m}"),
            IoError::Parse(m) => write!(f, "parse error: {m}"),
            IoError::Write(m) => write!(f, "write error: {m}"),
        }
    }
}

impl std::error::Error for IoError {}

/// Read and parse a save file from disk.
pub fn load_save(path: &Path) -> Result<Save, IoError> {
    let text = fs::read_to_string(path).map_err(|e| IoError::Read(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| IoError::Parse(e.to_string()))
}

/// Serialize a save to the exact text the Python engine would produce.
///
/// Matches `json.dumps(data, indent=4, ensure_ascii=False)`: 4-space indent,
/// `": "` after keys, `,` item separators with newlines, raw UTF-8 (no `\uXXXX`
/// escaping of non-ASCII), and **no trailing newline**.
pub fn serialize_pretty(data: &Save) -> Result<String, IoError> {
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 20);
    // Four-space indent to match Python's indent=4.
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    data.serialize(&mut ser)
        .map_err(|e| IoError::Write(e.to_string()))?;
    String::from_utf8(buf).map_err(|e| IoError::Write(e.to_string()))
}

/// Write `data` to `save_path`, optionally backing up the existing file first
/// and pruning old backups. The write is atomic (temp file + rename).
///
/// Returns the path of the backup that was created, if any.
pub fn write_save(
    save_path: &Path,
    data: &Save,
    make_backup: bool,
    keep_backups: Option<usize>,
) -> Result<Option<PathBuf>, IoError> {
    let mut backup: Option<PathBuf> = None;
    if make_backup && save_path.exists() {
        backup = Some(make_timestamped_backup(save_path)?);
    }
    let text = serialize_pretty(data)?;
    atomic_write_text(save_path, &text)?;
    if make_backup {
        if let Some(keep) = keep_backups {
            if keep > 0 {
                prune_old_backups(save_path, keep, backup.as_deref());
            }
        }
    }
    Ok(backup)
}

/// Copy `save_path` to `<name>.<YYYY-MM-DD_HHMMSS>.bak`, appending a numeric
/// suffix on same-second collisions so a rapid second save never clobbers the
/// previous backup (mirrors the Python `_make_timestamped_backup`).
pub fn make_timestamped_backup(save_path: &Path) -> Result<PathBuf, IoError> {
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    let file_name = save_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = save_path.parent().unwrap_or_else(|| Path::new("."));
    let base = parent.join(format!("{file_name}.{stamp}.bak"));
    let mut backup = base.clone();
    let mut n = 1u32;
    while backup.exists() {
        backup = parent.join(format!("{file_name}.{stamp}.bak{n}"));
        n += 1;
    }
    fs::copy(save_path, &backup).map_err(|e| IoError::Write(e.to_string()))?;
    Ok(backup)
}

/// Write `text` to `target` atomically via a sibling temp file + rename, with a
/// best-effort flush+sync so a crash mid-write can't truncate the save.
pub fn atomic_write_text(target: &Path, text: &str) -> Result<(), IoError> {
    let pid = std::process::id();
    let file_name = target
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!("{file_name}.tmp.{pid}"));
    let result = (|| -> Result<(), IoError> {
        let mut fh = fs::File::create(&tmp).map_err(|e| IoError::Write(e.to_string()))?;
        fh.write_all(text.as_bytes())
            .map_err(|e| IoError::Write(e.to_string()))?;
        fh.flush().map_err(|e| IoError::Write(e.to_string()))?;
        let _ = fh.sync_all();
        drop(fh);
        fs::rename(&tmp, target).map_err(|e| IoError::Write(e.to_string()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// All `<save_path>.<...>.bak` files, newest-first by modification time.
pub fn list_save_backups(save_path: &Path) -> Vec<PathBuf> {
    let parent = match save_path.parent() {
        Some(p) if p.as_os_str().is_empty() => Path::new("."),
        Some(p) => p,
        None => Path::new("."),
    };
    let prefix = save_path
        .file_name()
        .map(|s| format!("{}.", s.to_string_lossy()))
        .unwrap_or_default();
    let mut out: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&prefix) || !name.ends_with(".bak") {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            out.push((path, mtime));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out.into_iter().map(|(p, _)| p).collect()
}

/// Delete backups beyond the newest `keep`; `protect` (if any) is never deleted.
pub fn prune_old_backups(save_path: &Path, keep: usize, protect: Option<&Path>) -> Vec<PathBuf> {
    let backups = list_save_backups(save_path);
    let mut deleted = Vec::new();
    let protect_canon = protect.and_then(|p| fs::canonicalize(p).ok());
    for old in backups.into_iter().skip(keep) {
        if let Some(ref pc) = protect_canon {
            if fs::canonicalize(&old).ok().as_ref() == Some(pc) {
                continue;
            }
        }
        if fs::remove_file(&old).is_ok() {
            deleted.push(old);
        }
    }
    deleted
}

/// Default save location: `%USERPROFILE%/AppData/LocalLow/OrderOfMeta/Cargo Hunters/offline.save`.
pub fn default_save_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join("AppData")
            .join("LocalLow")
            .join("OrderOfMeta")
            .join("Cargo Hunters")
            .join("offline.save")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The exact 4-space style and the byte-faithful number handling are the
    // whole ballgame. This pins them against the cases that actually bit us.
    #[test]
    fn serialize_matches_python_style_and_preserves_numbers() {
        // Numbers as a STRING so the test asserts arbitrary_precision keeps the
        // literal verbatim - no f64 round-trip, no >2^53 loss.
        let src = concat!(
            "{\n",
            "    \"ulp_float\": 3.7905483245849609,\n",   // Python would shorten this
            "    \"big_int\": 540298907866066944,\n",      // > 2^53
            "    \"neg\": -1,\n",
            "    \"cond\": 4.0,\n",
            "    \"empty_obj\": {},\n",
            "    \"empty_arr\": [],\n",
            "    \"unicode\": \"café - ✓\",\n",
            "    \"nested\": {\n",
            "        \"b\": 1,\n",
            "        \"a\": 2\n",                            // key order must NOT sort
            "    }\n",
            "}"
        );
        let v: Save = serde_json::from_str(src).expect("parse");
        let out = serialize_pretty(&v).expect("serialize");
        assert_eq!(out, src, "round-trip must be byte-identical (4-space, raw unicode, key order, exact numbers)");
    }

    #[test]
    fn idempotent_fixed_point() {
        let src = "{\n    \"x\": 0.014999999664723873,\n    \"y\": [\n        1,\n        2\n    ]\n}";
        let v: Save = serde_json::from_str(src).unwrap();
        let a = serialize_pretty(&v).unwrap();
        let b = serialize_pretty(&serde_json::from_str::<Save>(&a).unwrap()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, src);
    }
}
