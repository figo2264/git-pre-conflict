use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::git::MergeTreeResult;

/// Type of conflict detected by git merge-tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    Content,
    AddAdd,
    ModifyDelete,
    DeleteModify,
    RenameDelete,
    RenameRename,
    DirectoryFile,
    Unknown,
}

impl std::fmt::Display for ConflictType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConflictType::Content => write!(f, "content"),
            ConflictType::AddAdd => write!(f, "add/add"),
            ConflictType::ModifyDelete => write!(f, "modify/delete"),
            ConflictType::DeleteModify => write!(f, "delete/modify"),
            ConflictType::RenameDelete => write!(f, "rename/delete"),
            ConflictType::RenameRename => write!(f, "rename/rename"),
            ConflictType::DirectoryFile => write!(f, "directory/file"),
            ConflictType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Details about a single conflicted file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictDetail {
    pub path: String,
    pub conflict_type: ConflictType,
    pub message: String,
}

/// Summary of a merge-tree conflict check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictReport {
    /// Current branch name
    pub current_branch: String,
    /// Target branch/ref that was checked against
    pub target_ref: String,
    /// Conflicted files with type and message details
    pub conflicted_files: Vec<ConflictDetail>,
}

impl ConflictReport {
    pub fn is_clean(&self) -> bool {
        self.conflicted_files.is_empty()
    }

    pub fn file_count(&self) -> usize {
        self.conflicted_files.len()
    }
}

/// Parse the conflict type from the parenthesized portion of a CONFLICT line.
fn parse_conflict_type(type_str: &str) -> ConflictType {
    match type_str.to_lowercase().as_str() {
        "content" => ConflictType::Content,
        "add/add" => ConflictType::AddAdd,
        "modify/delete" => ConflictType::ModifyDelete,
        "delete/modify" => ConflictType::DeleteModify,
        "rename/delete" => ConflictType::RenameDelete,
        "rename/rename" => ConflictType::RenameRename,
        "directory/file" | "file/directory" => ConflictType::DirectoryFile,
        _ => ConflictType::Unknown,
    }
}

/// Extract the file path from the message portion after `): `.
fn extract_path_from_message(message: &str) -> Option<String> {
    // "Merge conflict in <path>"
    if let Some(path) = message.strip_prefix("Merge conflict in ") {
        let path = path.trim();
        if !path.is_empty() {
            return Some(path.to_string());
        }
    }

    // Try to extract first path-like token from the message.
    // e.g. "<path> deleted in HEAD and modified in ..."
    // e.g. "<path> renamed to ..."
    let first_token = message.split_whitespace().next()?;
    if !first_token.is_empty() {
        return Some(first_token.to_string());
    }

    None
}

/// Parse the output of `git merge-tree --write-tree` to extract conflicted file details.
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

    let mut files: BTreeMap<String, ConflictDetail> = BTreeMap::new();

    for line in result.stdout.lines() {
        // Match "CONFLICT (<type>): <message>"
        if let Some(rest) = line.strip_prefix("CONFLICT (") {
            if let Some(paren_end) = rest.find(')') {
                let type_str = &rest[..paren_end];
                let conflict_type = parse_conflict_type(type_str);

                // Message is after "): "
                let message = rest[paren_end + 1..].trim().strip_prefix(':').map_or_else(
                    || rest[paren_end + 1..].trim().to_string(),
                    |m| m.trim().to_string(),
                );

                if let Some(path) = extract_path_from_message(&message) {
                    files.insert(
                        path.clone(),
                        ConflictDetail {
                            path,
                            conflict_type,
                            message: message.clone(),
                        },
                    );
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
                        // Only insert if not already present from CONFLICT line
                        files
                            .entry(path.to_string())
                            .or_insert_with(|| ConflictDetail {
                                path: path.to_string(),
                                conflict_type: ConflictType::Unknown,
                                message: String::new(),
                            });
                    }
                }
            }
        }
    }

    Ok(ConflictReport {
        current_branch,
        target_ref,
        conflicted_files: files.into_values().collect(),
    })
}
