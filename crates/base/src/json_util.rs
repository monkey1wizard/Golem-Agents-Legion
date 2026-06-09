//! JSON map utilities shared across GAL crates.
//!
//! Ports `Read-JsonOrderedMap`, `Write-JsonOrderedMap`, and `Merge-OrderedMap`
//! from `scripts/common/Common.{ps1,sh}`.

use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::Path;

/// Read a JSON file as an object map.
///
/// Returns an empty map when the file does not exist or is empty.
/// Returns `Err` when the file exists but cannot be parsed as a JSON object.
pub fn read_json_map(path: &Path) -> io::Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }

    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(Map::new());
    }

    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    match value {
        Value::Object(map) => Ok(map),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("JSON file is not an object: {}", path.display()),
        )),
    }
}

/// Write a JSON object map to a file (UTF-8, no BOM, pretty-printed).
///
/// Creates parent directories if they do not exist.
pub fn write_json_map(path: &Path, data: &Map<String, Value>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&Value::Object(data.clone()))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

/// Deep-merge `overlay` into `base` in-place.
///
/// When both values are objects, merges recursively.
/// Otherwise, overlay replaces base for that key.
///
/// Ports `Merge-OrderedMap` from Common.ps1.
pub fn merge_json_map(base: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, overlay_val) in overlay {
        match (base.get_mut(key), overlay_val) {
            (Some(Value::Object(base_obj)), Value::Object(overlay_obj)) => {
                merge_json_map(base_obj, overlay_obj);
            }
            _ => {
                base.insert(key.clone(), overlay_val.clone());
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::{NamedTempFile, TempDir};

    // ── read_json_map ─────────────────────────────────────────────────────────

    #[test]
    fn read_missing_file_returns_empty_map() {
        let m = read_json_map(Path::new("/nonexistent/t010-path.json")).unwrap();
        assert!(m.is_empty());
    }

    #[test]
    fn read_empty_file_returns_empty_map() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "").unwrap();
        let m = read_json_map(f.path()).unwrap();
        assert!(m.is_empty());
    }

    #[test]
    fn read_valid_object_returns_entries() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"key":"value","num":42}}"#).unwrap();
        let m = read_json_map(f.path()).unwrap();
        assert_eq!(m["key"], Value::String("value".into()));
        assert_eq!(m["num"], Value::Number(42.into()));
    }

    #[test]
    fn read_non_object_returns_error() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "[1,2,3]").unwrap();
        assert!(read_json_map(f.path()).is_err());
    }

    #[test]
    fn read_invalid_json_returns_error() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "{{ bad json ]]").unwrap();
        assert!(read_json_map(f.path()).is_err());
    }

    // ── write_json_map ────────────────────────────────────────────────────────

    #[test]
    fn write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sub").join("nested").join("out.json");
        let mut m = Map::new();
        m.insert("x".into(), Value::Number(1.into()));
        write_json_map(&path, &m).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn write_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data.json");
        let mut m = Map::new();
        m.insert("hello".into(), Value::String("world".into()));
        m.insert("count".into(), Value::Number(3.into()));
        write_json_map(&path, &m).unwrap();
        let back = read_json_map(&path).unwrap();
        assert_eq!(back["hello"], Value::String("world".into()));
        assert_eq!(back["count"], Value::Number(3.into()));
    }

    // ── merge_json_map ────────────────────────────────────────────────────────

    #[test]
    fn merge_overlay_replaces_scalar() {
        let mut base = Map::new();
        base.insert("a".into(), Value::Number(1.into()));
        base.insert("b".into(), Value::Number(2.into()));
        let mut overlay = Map::new();
        overlay.insert("b".into(), Value::Number(99.into()));
        overlay.insert("c".into(), Value::Number(3.into()));
        merge_json_map(&mut base, &overlay);
        assert_eq!(base["a"], Value::Number(1.into()));
        assert_eq!(base["b"], Value::Number(99.into()));
        assert_eq!(base["c"], Value::Number(3.into()));
    }

    #[test]
    fn merge_deep_objects_recursively() {
        let base_json = r#"{"outer":{"x":1,"y":2}}"#;
        let overlay_json = r#"{"outer":{"y":99,"z":3}}"#;
        let mut base: Map<String, Value> = serde_json::from_str(base_json).unwrap();
        let overlay: Map<String, Value> = serde_json::from_str(overlay_json).unwrap();
        merge_json_map(&mut base, &overlay);
        let outer = base["outer"].as_object().unwrap();
        assert_eq!(outer["x"], Value::Number(1.into()));
        assert_eq!(outer["y"], Value::Number(99.into()));
        assert_eq!(outer["z"], Value::Number(3.into()));
    }

    #[test]
    fn merge_overlay_into_empty_base() {
        let mut base = Map::new();
        let mut overlay = Map::new();
        overlay.insert("k".into(), Value::Bool(true));
        merge_json_map(&mut base, &overlay);
        assert_eq!(base["k"], Value::Bool(true));
    }

    #[test]
    fn merge_empty_overlay_leaves_base_unchanged() {
        let mut base = Map::new();
        base.insert("k".into(), Value::Bool(true));
        merge_json_map(&mut base, &Map::new());
        assert_eq!(base["k"], Value::Bool(true));
    }
}
