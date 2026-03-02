use std::process;

use clap::{Parser, Subcommand};
use colored::Colorize;

use git_pre_conflict_core::{conflict, git};

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
    },
    /// List all branches (local + remote)
    Branches,
}

fn main() {
    let cli = Cli::parse();
    let repo = cli.repo.as_deref();

    let code = match cli.command {
        Commands::Check { target, no_fetch } => run_check(repo, &target, no_fetch),
        Commands::Branches => run_branches(repo),
    };

    process::exit(code);
}

fn run_check(repo_path: Option<&str>, target: &str, no_fetch: bool) -> i32 {
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
        0
    } else {
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
        for path in &report.conflicted_files {
            println!("    {}", path.yellow());
        }
        1
    }
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
