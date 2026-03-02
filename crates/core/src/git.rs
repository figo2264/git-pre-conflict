use std::process::Command;

use crate::error::AppError;

/// Run a git command and return its stdout. Returns Err on non-zero exit
/// unless `allow_failure` is true.
/// When `repo_path` is Some, prepends `-C <path>` so git operates on that repo.
fn run_git(
    repo_path: Option<&str>,
    args: &[&str],
    allow_failure: bool,
) -> Result<(String, i32), AppError> {
    let mut cmd = Command::new("git");

    if let Some(path) = repo_path {
        cmd.args(["-C", path]);
    }

    let output = cmd
        .args(args)
        .output()
        .map_err(|e| AppError::GitCommand(format!("failed to run git: {e}")))?;

    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !allow_failure && !output.status.success() {
        return Err(AppError::GitCommand(format!(
            "git {} exited {code}: {stderr}",
            args.join(" ")
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok((stdout, code))
}

/// Verify we're inside a git repository. Returns the .git directory path.
pub fn find_git_dir(repo_path: Option<&str>) -> Result<String, AppError> {
    let (out, _) = run_git(repo_path, &["rev-parse", "--git-dir"], false)?;
    let dir = out.trim().to_string();
    if dir.is_empty() {
        return Err(AppError::NotARepo);
    }
    Ok(dir)
}

/// Get the current branch name.
pub fn current_branch(repo_path: Option<&str>) -> Result<String, AppError> {
    let (out, _) = run_git(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"], false)?;
    let branch = out.trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        return Err(AppError::GitCommand(
            "detached HEAD — cannot determine current branch".into(),
        ));
    }
    Ok(branch)
}

/// Fetch a specific branch from origin.
pub fn fetch_origin(repo_path: Option<&str>, target: &str) -> Result<(), AppError> {
    run_git(repo_path, &["fetch", "origin", target], false)?;
    Ok(())
}

/// Resolve the target ref: try origin/<target> first, fall back to <target>.
pub fn resolve_target_ref(repo_path: Option<&str>, target: &str) -> Result<String, AppError> {
    let remote_ref = format!("origin/{target}");

    let (_, code) = run_git(repo_path, &["rev-parse", "--verify", &remote_ref], true)?;
    if code == 0 {
        return Ok(remote_ref);
    }

    let (_, code) = run_git(repo_path, &["rev-parse", "--verify", target], true)?;
    if code == 0 {
        return Ok(target.to_string());
    }

    Err(AppError::GitCommand(format!(
        "cannot resolve ref: {target} (tried origin/{target} and {target})"
    )))
}

/// Result of `git merge-tree --write-tree`.
pub struct MergeTreeResult {
    /// Raw stdout from git merge-tree
    pub stdout: String,
    /// true if conflicts were detected (exit code 1)
    pub has_conflicts: bool,
}

/// Run `git merge-tree --write-tree <ours> <theirs>` to simulate a merge.
/// Exit code 0 = clean merge, 1 = conflicts detected.
pub fn merge_tree(
    repo_path: Option<&str>,
    ours: &str,
    theirs: &str,
) -> Result<MergeTreeResult, AppError> {
    let (stdout, code) = run_git(
        repo_path,
        &["merge-tree", "--write-tree", ours, theirs],
        true,
    )?;

    match code {
        0 => Ok(MergeTreeResult {
            stdout,
            has_conflicts: false,
        }),
        1 => Ok(MergeTreeResult {
            stdout,
            has_conflicts: true,
        }),
        other => Err(AppError::GitCommand(format!(
            "git merge-tree exited with unexpected code {other}"
        ))),
    }
}

/// List all branches (local + remote), filtering out `origin/HEAD`.
pub fn list_branches(repo_path: Option<&str>) -> Result<Vec<String>, AppError> {
    let (out, _) = run_git(
        repo_path,
        &["branch", "-a", "--format=%(refname:short)"],
        false,
    )?;

    let branches: Vec<String> = out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l != "origin/HEAD")
        .collect();

    Ok(branches)
}
