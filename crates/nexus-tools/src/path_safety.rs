use std::path::{Path, PathBuf};

/// Resolve a user-supplied path against project root and ensure it stays within root.
pub fn resolve_path_within_root(
    project_root: &Path,
    user_path: &str,
) -> Result<PathBuf, String> {
    let root = project_root
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize project root: {}", e))?;

    let joined = root.join(user_path);

    if joined.exists() {
        let canonical = joined
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
        if !canonical.starts_with(&root) {
            return Err("Path is outside the project root".to_string());
        }
        return Ok(canonical);
    }

    let parent = joined
        .parent()
        .ok_or_else(|| "Invalid path".to_string())?;

    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("Parent directory does not exist: {}", e))?;

    if !canonical_parent.starts_with(&root) {
        return Err("Path is outside the project root".to_string());
    }

    let file_name = joined
        .file_name()
        .ok_or_else(|| "Invalid file path".to_string())?;

    Ok(canonical_parent.join(file_name))
}
