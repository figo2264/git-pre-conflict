use std::process;

use clap::{Parser, Subcommand};
use colored::Colorize;

use git_pre_conflict_core::{conflict, git, guide};

#[derive(Parser)]
#[command(name = "git-pre-conflict")]
#[command(about = "Detect merge conflicts before they happen")]
#[command(version)]
struct Cli {
    /// Path to the git repository (defaults to current directory)
    #[arg(short = 'C', long = "repo", global = true)]
    repo: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// One-shot conflict check against a target branch
    Check {
        /// Target branch to check against (e.g. main, develop)
        target: String,

        /// Skip fetching from remote before checking
        #[arg(long)]
        no_fetch: bool,

        /// Show per-file diffs and resolution advice
        #[arg(long)]
        detail: bool,
    },
    /// List all branches (local + remote)
    Branches,
}

fn main() {
    let cli = Cli::parse();
    let repo = cli.repo.as_deref();

    let code = match cli.command {
        Commands::Check {
            target,
            no_fetch,
            detail,
        } => run_check(repo, &target, no_fetch, detail),
        Commands::Branches => run_branches(repo),
    };

    process::exit(code);
}

fn run_check(repo_path: Option<&str>, target: &str, no_fetch: bool, detail: bool) -> i32 {
    if let Err(e) = git::find_git_dir(repo_path) {
        eprintln!("{} {e}", "error:".red().bold());
        return 2;
    }

    let current = match git::current_branch(repo_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 2;
        }
    };

    if !no_fetch {
        if let Err(e) = git::fetch_origin(repo_path, target) {
            eprintln!("{} {e}", "warning:".yellow().bold());
        }
    }

    let target_ref = match git::resolve_target_ref(repo_path, target) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 2;
        }
    };

    let result = match git::merge_tree(repo_path, &current, &target_ref) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 2;
        }
    };

    let report = match conflict::parse_merge_tree(&result, current, target_ref) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 2;
        }
    };

    // Print results
    println!(
        "{}  {} -> {}",
        "branch:".bold(),
        report.current_branch.cyan(),
        report.target_ref.cyan()
    );

    if report.is_clean() {
        println!("{}", "  No conflicts detected.".green().bold());
        return 0;
    }

    println!(
        "{}",
        format!(
            "  {} conflicted file{}:",
            report.file_count(),
            if report.file_count() == 1 { "" } else { "s" }
        )
        .red()
        .bold()
    );

    for detail_item in &report.conflicted_files {
        let badge = format!("[{}]", detail_item.conflict_type).dimmed();
        println!("    {} {}", badge, detail_item.path.yellow());
    }

    // Resolution guide
    let res_guide = guide::generate_resolution_guide(
        &report.current_branch,
        &report.target_ref,
        &report.conflicted_files,
    );

    println!();
    println!("{}", "  Resolution guide:".bold());
    println!("  {}", res_guide.summary.dimmed());
    println!();

    for (i, step) in res_guide.commands.iter().enumerate() {
        println!(
            "    {}. {} {}",
            i + 1,
            step.description,
            step.command.cyan()
        );
    }

    // --detail: show per-file diffs
    if detail {
        let merge_base = git::merge_base(repo_path, &report.current_branch, &report.target_ref);

        if let Ok(base) = merge_base {
            println!();
            println!("{}", "  Per-file details:".bold());

            for file_guide in &res_guide.per_file_advice {
                println!();
                println!("  {} {}", "---".dimmed(), file_guide.path.yellow().bold());
                println!("  {}", file_guide.advice.dimmed());

                match git::diff_file(repo_path, &base, &report.target_ref, &file_guide.path) {
                    Ok(diff) if !diff.is_empty() => {
                        println!();
                        for (n, line) in diff.lines().enumerate() {
                            if n >= 50 {
                                println!("    {}", "... (truncated)".dimmed());
                                break;
                            }
                            let colored = if line.starts_with('+') {
                                line.green().to_string()
                            } else if line.starts_with('-') {
                                line.red().to_string()
                            } else if line.starts_with("@@") {
                                line.purple().to_string()
                            } else {
                                line.to_string()
                            };
                            println!("    {colored}");
                        }
                    }
                    Ok(_) => {
                        println!("    {}", "(no diff available)".dimmed());
                    }
                    Err(e) => {
                        println!("    {} {e}", "diff error:".red());
                    }
                }
            }
        } else if let Err(e) = merge_base {
            eprintln!(
                "{} could not compute merge base: {e}",
                "warning:".yellow().bold()
            );
        }
    }

    1
}

fn run_branches(repo_path: Option<&str>) -> i32 {
    if let Err(e) = git::find_git_dir(repo_path) {
        eprintln!("{} {e}", "error:".red().bold());
        return 2;
    }

    match git::list_branches(repo_path) {
        Ok(branches) => {
            for b in &branches {
                println!("{b}");
            }
            0
        }
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            2
        }
    }
}
