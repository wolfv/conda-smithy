//! `smithy` — the conda-smithy rewrite in Rust.
//!
//! ```text
//! smithy lint       # lint the recipe + conda-forge.yml
//! smithy rerender   # regenerate CI configuration from templates
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use smithy_core::{lint, render, Feedstock, Severity};

#[derive(Parser)]
#[command(
    name = "smithy",
    version,
    about = "Lint conda recipes and render feedstock CI — scriptable with Rhai lint rules and minijinja templates"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lint the recipe and conda-forge.yml of a feedstock.
    ///
    /// Runs the built-in rules plus any *.rhai scripts found in
    /// .smithy/lints/. Exits non-zero when lints (errors) are found.
    Lint {
        /// Path to the feedstock directory.
        #[arg(long, default_value = ".")]
        feedstock_dir: PathBuf,
    },
    /// (Re-)generate CI configuration from templates.
    ///
    /// Renders the built-in minijinja templates (overridable via
    /// .smithy/templates/) plus any extra user templates, and writes
    /// .ci_support/ variant files.
    Rerender {
        /// Path to the feedstock directory.
        #[arg(long, default_value = ".")]
        feedstock_dir: PathBuf,
        /// Print what would be written without writing anything.
        #[arg(long)]
        check: bool,
    },
}

fn run() -> Result<ExitCode> {
    match Cli::parse().command {
        Command::Lint { feedstock_dir } => {
            let feedstock = Feedstock::load(&feedstock_dir)?;
            let report = lint::lint_feedstock(&feedstock)?;

            for message in &report.messages {
                let tag = match message.severity {
                    Severity::Lint => "error",
                    Severity::Hint => "hint ",
                };
                println!("{tag} [{}] {}", message.rule, message.message);
            }
            let (lints, hints) = (report.lints().count(), report.hints().count());
            if lints == 0 && hints == 0 {
                println!(
                    "✓ {} looks good — no lints, no hints",
                    feedstock.recipe.path.display()
                );
            } else {
                println!("\n{lints} lint(s), {hints} hint(s)");
            }
            Ok(if report.has_lints() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Rerender {
            feedstock_dir,
            check,
        } => {
            let feedstock = Feedstock::load(&feedstock_dir)?;
            if check {
                for file in render::render_feedstock(&feedstock)? {
                    println!("would write {}", file.path.display());
                }
            } else {
                for path in render::rerender(&feedstock)? {
                    println!("wrote {}", path.display());
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
