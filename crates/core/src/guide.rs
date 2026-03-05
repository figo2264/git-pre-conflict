use serde::{Deserialize, Serialize};

use crate::conflict::{ConflictDetail, ConflictType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionGuide {
    pub summary: String,
    pub commands: Vec<GuideStep>,
    pub per_file_advice: Vec<FileGuide>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideStep {
    pub description: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileGuide {
    pub path: String,
    pub conflict_type: ConflictType,
    pub advice: String,
}

/// Generate a step-by-step resolution guide for the detected conflicts.
pub fn generate_resolution_guide(
    current_branch: &str,
    target_ref: &str,
    conflicts: &[ConflictDetail],
) -> ResolutionGuide {
    let summary = format!(
        "To resolve: merge {} into your working branch ({}).",
        target_ref, current_branch,
    );

    let mut commands = vec![
        GuideStep {
            description: "Make sure you are on your working branch".to_string(),
            command: format!("git checkout {}", current_branch),
        },
        GuideStep {
            description: "Fetch latest changes from remote".into(),
            command: "git fetch origin".into(),
        },
        GuideStep {
            description: "Start the merge (this will show conflicts)".to_string(),
            command: format!("git merge {}", target_ref),
        },
    ];

    for detail in conflicts {
        commands.push(GuideStep {
            description: format!("Resolve conflict in {}", detail.path),
            command: format!("git add {}", detail.path),
        });
    }

    commands.push(GuideStep {
        description: "Complete the merge".into(),
        command: "git commit".into(),
    });

    let per_file_advice = conflicts
        .iter()
        .map(|d| FileGuide {
            path: d.path.clone(),
            conflict_type: d.conflict_type.clone(),
            advice: advice_for_type(&d.conflict_type, &d.path),
        })
        .collect();

    ResolutionGuide {
        summary,
        commands,
        per_file_advice,
    }
}

/// Return human-readable advice for a given conflict type.
pub fn advice_for_type(conflict_type: &ConflictType, path: &str) -> String {
    match conflict_type {
        ConflictType::Content => format!(
            "Both branches modified '{}'. Open the file and look for <<<<<<< / ======= / >>>>>>> markers. \
             Edit the file to keep the correct changes, then save.",
            path
        ),
        ConflictType::AddAdd => format!(
            "Both branches added '{}'. Decide which version to keep, or merge the contents manually.",
            path
        ),
        ConflictType::ModifyDelete => format!(
            "One branch modified '{}' while the other deleted it. \
             Run `git add {}` to keep the file, or `git rm {}` to delete it.",
            path, path, path
        ),
        ConflictType::DeleteModify => format!(
            "One branch deleted '{}' while the other modified it. \
             Run `git add {}` to keep the file, or `git rm {}` to delete it.",
            path, path, path
        ),
        ConflictType::RenameDelete => format!(
            "One branch renamed '{}' while the other deleted it. \
             Decide whether to keep the renamed file or accept the deletion.",
            path
        ),
        ConflictType::RenameRename => format!(
            "Both branches renamed '{}' to different names. \
             Choose the correct name and update accordingly.",
            path
        ),
        ConflictType::DirectoryFile => format!(
            "A directory/file conflict exists at '{}'. \
             One branch has a file where the other has a directory. Reorganize as needed.",
            path
        ),
        ConflictType::Unknown => format!(
            "Resolve '{}' manually. Open the file and look for conflict markers if present.",
            path
        ),
    }
}
