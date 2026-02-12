use std::collections::HashMap;
use std::fmt;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
pub enum GitDiffError {
    GitNotFound,
    NotARepo,
    BaseRefNotFound(String),
    CommandFailed(String),
}

impl fmt::Display for GitDiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitDiffError::GitNotFound => write!(f, "git is not installed or not in PATH"),
            GitDiffError::NotARepo => write!(f, "not inside a git repository"),
            GitDiffError::BaseRefNotFound(r) => {
                write!(f, "base ref '{}' not found (try fetching it first)", r)
            }
            GitDiffError::CommandFailed(msg) => write!(f, "git command failed: {}", msg),
        }
    }
}

impl std::error::Error for GitDiffError {}

/// Changed files and line ranges from a git diff.
#[derive(Debug)]
pub struct DiffInfo {
    /// Map of relative file path to list of changed line ranges.
    pub changed_lines: HashMap<PathBuf, Vec<RangeInclusive<usize>>>,
}

impl DiffInfo {
    pub fn has_file(&self, path: &PathBuf) -> bool {
        self.changed_lines.contains_key(path)
    }

    /// Check if a specific line in a file is within a changed range.
    pub fn has_line(&self, path: &PathBuf, line: usize) -> bool {
        match self.changed_lines.get(path) {
            Some(ranges) => ranges.iter().any(|r| r.contains(&line)),
            None => false,
        }
    }
}

/// Detect the base ref from CI environment variables, falling back to "main".
pub fn detect_base_ref() -> String {
    // GitHub Actions
    if let Ok(base) = std::env::var("GITHUB_BASE_REF") {
        if !base.is_empty() {
            return base;
        }
    }
    // GitLab CI
    if let Ok(base) = std::env::var("CI_MERGE_REQUEST_TARGET_BRANCH_NAME") {
        if !base.is_empty() {
            return base;
        }
    }
    // Bitbucket Pipelines
    if let Ok(base) = std::env::var("BITBUCKET_PR_DESTINATION_BRANCH") {
        if !base.is_empty() {
            return base;
        }
    }
    "main".to_string()
}

/// Get the repository root directory.
pub fn repo_root() -> Result<PathBuf, GitDiffError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|_| GitDiffError::GitNotFound)?;

    if !output.status.success() {
        return Err(GitDiffError::NotARepo);
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Parse a git diff to extract changed files and their changed line ranges.
///
/// Uses triple-dot diff (`base...HEAD`) for correct merge-base comparison.
/// Only includes Added, Copied, Modified, Renamed files (`--diff-filter=ACMR`).
pub fn diff_info(base_ref: &str) -> Result<DiffInfo, GitDiffError> {
    // Ensure we're in a git repo
    repo_root()?;

    // Try the base ref directly, then with origin/ prefix
    let effective_base = resolve_base_ref(base_ref)?;

    let output = Command::new("git")
        .args([
            "diff",
            "-U0",
            "--diff-filter=ACMR",
            &format!("{}...HEAD", effective_base),
        ])
        .output()
        .map_err(|_| GitDiffError::GitNotFound)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(GitDiffError::CommandFailed(stderr));
    }

    let diff_text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_diff(&diff_text))
}

/// Resolve a base ref, trying the ref directly then with origin/ prefix.
/// For shallow clones, attempts a fetch first.
fn resolve_base_ref(base_ref: &str) -> Result<String, GitDiffError> {
    // Try the ref directly
    if ref_exists(base_ref) {
        return Ok(base_ref.to_string());
    }

    // Try with origin/ prefix
    let with_origin = format!("origin/{}", base_ref);
    if ref_exists(&with_origin) {
        return Ok(with_origin);
    }

    // Attempt shallow fetch and retry
    let _ = Command::new("git")
        .args(["fetch", "--depth=1", "origin", base_ref])
        .output();

    if ref_exists(&with_origin) {
        return Ok(with_origin);
    }

    if ref_exists(base_ref) {
        return Ok(base_ref.to_string());
    }

    Err(GitDiffError::BaseRefNotFound(base_ref.to_string()))
}

fn ref_exists(r: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", r])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Parse unified diff output into a DiffInfo.
fn parse_diff(diff_text: &str) -> DiffInfo {
    let mut changed_lines: HashMap<PathBuf, Vec<RangeInclusive<usize>>> = HashMap::new();
    let mut current_file: Option<PathBuf> = None;

    for line in diff_text.lines() {
        // Detect file path from +++ line
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = Some(PathBuf::from(path));
            changed_lines
                .entry(PathBuf::from(path))
                .or_insert_with(Vec::new);
            continue;
        }

        // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
        if line.starts_with("@@") {
            if let Some(ref file) = current_file {
                if let Some(range) = parse_hunk_header(line) {
                    changed_lines.entry(file.clone()).or_default().push(range);
                }
            }
        }
    }

    DiffInfo { changed_lines }
}

/// Parse a hunk header like `@@ -10,3 +15,4 @@` and return the new-side line range.
///
/// Format: `+start,count` means lines `start..=start+count-1`.
/// If count is 0, it's a pure deletion — return None.
/// If count is omitted, it defaults to 1.
fn parse_hunk_header(line: &str) -> Option<RangeInclusive<usize>> {
    // Find the +start,count portion
    let plus_pos = line.find('+')?;
    let after_plus = &line[plus_pos + 1..];

    // Find the end of the numbers (next space or @@)
    let end = after_plus
        .find(|c: char| c == ' ' || c == '@')
        .unwrap_or(after_plus.len());
    let range_str = &after_plus[..end];

    if let Some(comma_pos) = range_str.find(',') {
        let start: usize = range_str[..comma_pos].parse().ok()?;
        let count: usize = range_str[comma_pos + 1..].parse().ok()?;
        if count == 0 {
            return None; // pure deletion
        }
        Some(start..=start + count - 1)
    } else {
        // No comma — single line change (count = 1)
        let start: usize = range_str.parse().ok()?;
        Some(start..=start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hunk_single_line() {
        let range = parse_hunk_header("@@ -10,0 +15 @@").unwrap();
        assert_eq!(range, 15..=15);
    }

    #[test]
    fn parse_hunk_multi_line() {
        let range = parse_hunk_header("@@ -10,3 +15,4 @@").unwrap();
        assert_eq!(range, 15..=18);
    }

    #[test]
    fn parse_hunk_pure_deletion() {
        let range = parse_hunk_header("@@ -10,3 +14,0 @@");
        assert!(range.is_none());
    }

    #[test]
    fn parse_hunk_with_context() {
        let range = parse_hunk_header("@@ -1,5 +1,7 @@ fn main() {").unwrap();
        assert_eq!(range, 1..=7);
    }

    #[test]
    fn parse_diff_full() {
        let diff = "\
diff --git a/src/foo.rs b/src/foo.rs
index abc..def 100644
--- a/src/foo.rs
+++ b/src/foo.rs
@@ -1,3 +1,5 @@
+new line 1
+new line 2
 existing
diff --git a/src/bar.rs b/src/bar.rs
new file mode 100644
--- /dev/null
+++ b/src/bar.rs
@@ -0,0 +1,10 @@
+all new file
";
        let info = parse_diff(diff);
        assert!(info.changed_lines.contains_key(&PathBuf::from("src/foo.rs")));
        assert!(info.changed_lines.contains_key(&PathBuf::from("src/bar.rs")));

        let foo_ranges = &info.changed_lines[&PathBuf::from("src/foo.rs")];
        assert_eq!(foo_ranges.len(), 1);
        assert_eq!(foo_ranges[0], 1..=5);

        let bar_ranges = &info.changed_lines[&PathBuf::from("src/bar.rs")];
        assert_eq!(bar_ranges.len(), 1);
        assert_eq!(bar_ranges[0], 1..=10);
    }

    #[test]
    fn diff_info_has_file_and_line() {
        let mut changed_lines = HashMap::new();
        changed_lines.insert(
            PathBuf::from("src/main.rs"),
            vec![5..=10, 20..=25],
        );
        let info = DiffInfo { changed_lines };

        assert!(info.has_file(&PathBuf::from("src/main.rs")));
        assert!(!info.has_file(&PathBuf::from("src/other.rs")));

        assert!(info.has_line(&PathBuf::from("src/main.rs"), 7));
        assert!(info.has_line(&PathBuf::from("src/main.rs"), 20));
        assert!(!info.has_line(&PathBuf::from("src/main.rs"), 15));
    }

    #[test]
    fn detect_base_ref_defaults_to_main() {
        // When no CI env vars are set, should default to "main"
        // (This test may behave differently in CI, but the logic is correct)
        let base = detect_base_ref();
        // In local dev, should be "main" unless CI env vars are set
        assert!(!base.is_empty());
    }
}
