//! `smithy` — the conda-smithy rewrite in Rust.
//!
//! ```text
//! smithy lint       # lint the recipe + conda-forge.yml
//! smithy rerender   # regenerate CI configuration from templates
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use smithy_core::{lint, render, Feedstock, Severity};

mod init;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    /// Human-readable lines.
    Text,
    /// One JSON object: {"lints": [...], "hints": [...]}.
    Json,
}

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
        /// Output format; `json` is meant for CI integration.
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
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
    /// Create a new feedstock skeleton (conda-forge.yml, example recipe,
    /// .smithy/ extension points). Never overwrites existing files.
    Init {
        /// The package name the feedstock builds.
        name: String,
        /// Directory to initialise (created if missing).
        #[arg(long, default_value = ".")]
        feedstock_dir: PathBuf,
        /// Recipe format: v1 (rattler-build recipe.yaml) or v0
        /// (conda-build meta.yaml).
        #[arg(long, default_value = "v1")]
        recipe_format: String,
    },
}

fn run() -> Result<ExitCode> {
    match Cli::parse().command {
        Command::Lint {
            feedstock_dir,
            format,
        } => {
            let feedstock = Feedstock::load(&feedstock_dir)?;
            let report = lint::lint_feedstock(&feedstock)?;

            match format {
                OutputFormat::Text => {
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
                }
                OutputFormat::Json => {
                    let entry = |m: &lint::LintMessage| serde_json::json!({ "rule": m.rule, "message": m.message });
                    let payload = serde_json::json!({
                        "recipe": feedstock.recipe.path,
                        "lints": report.lints().map(entry).collect::<Vec<_>>(),
                        "hints": report.hints().map(entry).collect::<Vec<_>>(),
                    });
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                }
            }
            Ok(if report.has_lints() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Init {
            name,
            feedstock_dir,
            recipe_format,
        } => {
            let format = match recipe_format.as_str() {
                "v1" => init::RecipeFormat::V1,
                "v0" => init::RecipeFormat::V0,
                other => anyhow::bail!("unknown recipe format `{other}`; use v1 or v0"),
            };
            let created = init::init_feedstock(&feedstock_dir, &name, format)?;
            if created.is_empty() {
                println!("nothing to do — all files already exist");
            } else {
                for file in &created {
                    println!("created {file}");
                }
                println!(
                    "\nNext steps: fill in {}/recipe, then run `smithy rerender`.",
                    feedstock_dir.display()
                );
            }
            Ok(ExitCode::SUCCESS)
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
