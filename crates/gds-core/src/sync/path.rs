//! Path validation and conflict copy naming.
//!
//! SECURITY: Path traversal from Drive file names is prevented here.
//! Drive allows filenames like "../../../etc/passwd". All paths are
//! resolved relative to sync root and validated for containment.

use std::path::{Path, PathBuf};

use crate::model::SyncError;

/// Builds a safe local path under `sync_root` from a relative path string.
///
/// - Rejects null bytes and collapses ".." so the result is under `sync_root`.
/// - Returns `PathTraversal` if the resolved path would escape the sync root.
///
/// SECURITY: Call this for every path derived from Drive (e.g. file name) or
/// user input before using it for filesystem operations.
pub fn safe_local_path(sync_root: &Path, relative: &str) -> Result<PathBuf, SyncError> {
    if relative.contains('\0') {
        return Err(SyncError::PathTraversal {
            path: relative.to_string(),
        });
    }

    let mut segments = Vec::new();
    for seg in relative.split('/').filter(|c| !c.is_empty()) {
        if seg == ".." {
            segments.pop();
        } else if seg != "." {
            segments.push(seg);
        }
    }
    let sanitized = segments.join("/");

    let candidate = sync_root.join(&sanitized);

    let sync_root_canon = sync_root.canonicalize().map_err(|e| SyncError::IoError {
        path: sync_root.display().to_string(),
        source: e,
    })?;

    if candidate.exists() {
        let canonical = candidate.canonicalize().map_err(|e| SyncError::IoError {
            path: candidate.display().to_string(),
            source: e,
        })?;
        if !canonical.starts_with(&sync_root_canon) {
            return Err(SyncError::PathTraversal {
                path: relative.to_string(),
            });
        }
    } else {
        let mut ancestor = candidate.parent();
        while let Some(p) = ancestor {
            if p == sync_root || p == Path::new(".") {
                break;
            }
            if p.exists() {
                let p_canon = p.canonicalize().map_err(|e| SyncError::IoError {
                    path: p.display().to_string(),
                    source: e,
                })?;
                if !p_canon.starts_with(&sync_root_canon) {
                    return Err(SyncError::PathTraversal {
                        path: relative.to_string(),
                    });
                }
                break;
            }
            ancestor = p.parent();
        }
    }

    Ok(candidate)
}

/// Suffix format for conflict copies: `.conflict-YYYYMMDD-HHMMSS`.
pub const CONFLICT_SUFFIX_FORMAT: &str = ".conflict-%Y%m%d-%H%M%S";

/// Generates a conflict copy path for `base_path`, using `timestamp` for the suffix.
/// Collision-safe: if the path already exists, appends `-2`, `-3`, etc.
///
/// `base_path` is the full path of the file (e.g. `/home/user/Drive/doc.txt`).
/// Returns e.g. `doc.txt.conflict-20240115-120000` or `doc.txt.conflict-20240115-120000-2`.
pub fn conflict_copy_path(
    base_path: &Path,
    timestamp: chrono::DateTime<chrono::Utc>,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let parent = base_path.parent().unwrap_or(Path::new("."));
    let stem = base_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = base_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    let suffix = timestamp.format("%Y%m%d-%H%M%S").to_string();
    let conflict_name = format!("{}{}.conflict-{}{}", stem, ext, suffix, ext);
    let mut candidate = parent.join(&conflict_name);

    if !exists(&candidate) {
        return candidate;
    }

    let mut n = 2u32;
    loop {
        let name = format!("{}{}.conflict-{}-{}{}", stem, ext, suffix, n, ext);
        candidate = parent.join(&name);
        if !exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn safe_local_path_rejects_null_byte() {
        let root = Path::new("/home/user/Drive");
        let err = safe_local_path(root, "a\x00b").unwrap_err();
        assert!(matches!(err, SyncError::PathTraversal { .. }));
    }

    #[test]
    fn safe_local_path_strips_dot_dot() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let p = safe_local_path(root, "a/../b/./c").unwrap();
        assert_eq!(p, root.join("b/c"));
    }

    #[test]
    fn safe_local_path_accepts_normal_relative() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let p = safe_local_path(root, "docs/file.txt").unwrap();
        assert_eq!(p, root.join("docs/file.txt"));
    }

    #[test]
    fn conflict_copy_path_collision_safe() {
        let base = Path::new("/dir/file.txt");
        let ts = chrono::DateTime::parse_from_rfc3339("2024-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let exists = |p: &Path| p.to_str() == Some("/dir/file.txt.conflict-20240115-120000.txt");
        let out = conflict_copy_path(base, ts, exists);
        assert!(out.to_str().unwrap().ends_with("-2.txt"));
    }

    #[test]
    fn conflict_copy_path_no_collision() {
        let base = Path::new("/dir/file.txt");
        let ts = chrono::DateTime::parse_from_rfc3339("2024-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let out = conflict_copy_path(base, ts, |_| false);
        assert_eq!(
            out.to_str().unwrap(),
            "/dir/file.txt.conflict-20240115-120000.txt"
        );
    }
}
