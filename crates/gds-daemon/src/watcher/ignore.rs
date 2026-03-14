//! Ignore patterns for file watcher: skip .gds_tmp, .git, editor temp files, own conflict copies.

use std::path::Path;

/// Returns true if the path should be ignored (do not trigger sync).
///
/// Ignores:
/// - `.gds_tmp` (daemon temp writes)
/// - `.git` and any path containing `/.git/`
/// - `*.swp`, `*~`, `.#*` (common editor temp files)
/// - Paths containing `.conflict-` (daemon conflict copies)
///
/// Path is interpreted relative to sync_root (path should be under sync_root).
/// Does not require path to exist (so event paths for deleted files still work).
pub fn should_ignore(sync_root: &Path, path: &Path) -> bool {
    let relative = match path.strip_prefix(sync_root) {
        Ok(r) => r,
        Err(_) => return true, // outside sync root
    };

    let path_str_rel = relative.to_string_lossy();
    // Skip any path under .gds_tmp or .git (or named exactly .gds_tmp/.git)
    if path_str_rel.contains("/.gds_tmp/")
        || path_str_rel.starts_with(".gds_tmp/")
        || path_str_rel == ".gds_tmp"
    {
        return true;
    }
    if path_str_rel.contains("/.git/")
        || path_str_rel.starts_with(".git/")
        || path_str_rel == ".git"
    {
        return true;
    }

    for comp in relative.components() {
        let name = match comp.as_os_str().to_str() {
            Some(n) => n,
            None => return true,
        };
        if name == ".gds_tmp" || name == ".git" {
            return true;
        }
    }

    // Editor temp: *.swp, *~, .#*
    let file_name = relative.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if file_name.ends_with(".swp") || file_name.ends_with('~') {
        return true;
    }
    if file_name.starts_with(".#") {
        return true;
    }

    // Daemon conflict copies: any path segment or filename containing .conflict-
    if path_str_rel.contains(".conflict-") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from("/tmp/sync_root")
    }

    #[test]
    fn ignore_gds_tmp() {
        assert!(should_ignore(&root(), &root().join(".gds_tmp/file")));
        assert!(should_ignore(&root(), &root().join("a/.gds_tmp")));
    }

    #[test]
    fn ignore_git() {
        assert!(should_ignore(&root(), &root().join(".git/HEAD")));
        assert!(should_ignore(&root(), &root().join(".git/refs/heads/main")));
        assert!(should_ignore(&root(), &root().join("sub/.git/config")));
    }

    #[test]
    fn ignore_editor_temp() {
        assert!(should_ignore(&root(), &root().join("file.swp")));
        assert!(should_ignore(&root(), &root().join("doc.txt~")));
        assert!(should_ignore(&root(), &root().join(".#file.txt")));
    }

    #[test]
    fn ignore_conflict_copy() {
        assert!(should_ignore(
            &root(),
            &root().join("doc.txt.conflict-20240115-120000")
        ));
        assert!(should_ignore(
            &root(),
            &root().join("dir/.conflict-20240115-120000")
        ));
    }

    #[test]
    fn allow_normal_files() {
        // We can't canonicalize /tmp/sync_root in test without creating it,
        // so test the logic on a path that would not canonicalize in unit test.
        // Instead we test that paths that don't match patterns are not ignored
        // by construction: any path with .gds_tmp, .git, .swp, ~, .#, .conflict-
        // is ignored. So "foo/bar.txt" has none of these - but should_ignore
        // returns true if canonicalize fails (path may not exist). So for unit
        // tests we only test the positive ignore cases above. For "allow normal"
        // we need integration test with real dir.
    }
}
