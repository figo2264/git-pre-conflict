use std::collections::BTreeSet;

use serde::Serialize;

use crate::error::AppError;
use crate::git::MergeTreeResult;

/// Summary of a merge-tree conflict check.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictReport {
    /// Current branch name
    pub current_branch: String,
    /// Target branch/ref that was checked against
    pub target_ref: String,
    /// Conflicted file paths (sorted, deduplicated)
    pub conflicted_files: Vec<String>,
}

impl ConflictReport {
    pub fn is_clean(&self) -> bool {
        self.conflicted_files.is_empty()
    }

    pub fn file_count(&self) -> usize {
        self.conflicted_files.len()
    }
}

/// Parse the output of `git merge-tree --write-tree` to extract conflicted file paths.
pub fn parse_merge_tree(
    result: &MergeTreeResult,
    current_branch: String,
    target_ref: String,
) -> Result<ConflictReport, AppError> {
    if !result.has_conflicts {
        return Ok(ConflictReport {
            current_branch,
            target_ref,
            conflicted_files: Vec::new(),
        });
    }

    let mut files = BTreeSet::new();

    for line in result.stdout.lines() {
        // Match "CONFLICT (content): Merge conflict in <path>"
        if let Some(rest) = line.strip_prefix("CONFLICT") {
            if let Some(idx) = rest.find("Merge conflict in ") {
                let path = rest[idx + "Merge conflict in ".len()..].trim();
                if !path.is_empty() {
                    files.insert(path.to_string());
                }
            }
            continue;
        }

        // Handle tab-delimited lines: <mode> <hash> <stage>\t<path>
        // Stage > 0 indicates conflict.
        if let Some(tab_pos) = line.find('\t') {
            let meta = &line[..tab_pos];
            let path = line[tab_pos + 1..].trim();

            let parts: Vec<&str> = meta.split_whitespace().collect();
            if parts.len() >= 3 {
                if let Ok(stage) = parts[2].parse::<u32>() {
                    if stage > 0 && !path.is_empty() {
                        files.insert(path.to_string());
                    }
                }
            }
        }
    }

    Ok(ConflictReport {
        current_branch,
        target_ref,
        conflicted_files: files.into_iter().collect(),
    })
}
