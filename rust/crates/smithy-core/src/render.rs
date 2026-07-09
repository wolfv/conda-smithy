//! Rerendering: generate feedstock CI configuration from templates.
//!
//! The build matrix is computed in Rust from `conda-forge.yml`
//! ([`compute_matrix`]) and handed to [minijinja](https://docs.rs/minijinja)
//! templates as ready-to-print values, so the templates themselves stay
//! simple enough for anyone to edit.
//!
//! Template resolution:
//! 1. `.smithy/templates/<name>` in the feedstock (user override)
//! 2. the built-in template embedded in the binary
//!
//! Any *extra* `*.j2` file in `.smithy/templates/` becomes an additional
//! output: the `.j2` suffix is stripped and `__` turns into `/`, so
//! `.smithy/templates/.github__workflows__docs.yml.j2` renders to
//! `.github/workflows/docs.yml` with the same context.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use minijinja::Environment;
use serde::Serialize;

use crate::config::Provider;
use crate::feedstock::Feedstock;
use crate::recipe::lookup;
use crate::variants::{cell_name_suffix, UsedVars, VariantCell, VariantConfig};

/// Built-in templates and the file each one renders to.
const BUILTIN_TEMPLATES: &[(&str, &str)] = &[
    ("github-actions.yml.j2", ".github/workflows/conda-build.yml"),
    ("azure-pipelines.yml.j2", "azure-pipelines.yml"),
    ("README.md.j2", "README.md"),
];

fn builtin_template_source(name: &str) -> Option<&'static str> {
    match name {
        "github-actions.yml.j2" => Some(include_str!("../templates/github-actions.yml.j2")),
        "azure-pipelines.yml.j2" => Some(include_str!("../templates/azure-pipelines.yml.j2")),
        "README.md.j2" => Some(include_str!("../templates/README.md.j2")),
        _ => None,
    }
}

/// One entry of the build matrix — one CI job.
#[derive(Debug, Clone, Serialize)]
pub struct BuildConfig {
    /// e.g. `linux_64`, also the `.ci_support/<name>.yaml` file stem.
    pub name: String,
    /// Target platform in dash form, e.g. `linux-64`.
    pub target_platform: String,
    /// Platform the build runs on (differs when cross-compiling).
    pub build_platform: String,
    /// `linux`, `osx` or `win` (of the build platform).
    pub os: String,
    /// CI provider, e.g. `azure` or `github_actions`.
    pub provider: String,
    /// Whether built packages are uploaded from this job.
    pub upload: bool,
    /// GitHub Actions runner labels for this job.
    pub gha_runs_on: Vec<String>,
    /// Docker image for Linux builds.
    pub docker_image: Option<String>,
    pub cross_compile: bool,
    /// Variant values from `conda_build_config.yaml` / `variants.yaml`
    /// (e.g. `python` → `3.11`), written into `.ci_support/<name>.yaml`.
    pub variant: VariantCell,
}

/// Platforms conda-forge builds by default.
const DEFAULT_TARGET_PLATFORMS: &[&str] = &["linux_64", "osx_64", "win_64"];

fn os_of(platform: &str) -> &'static str {
    if platform.starts_with("linux") {
        "linux"
    } else if platform.starts_with("osx") {
        "osx"
    } else {
        "win"
    }
}

fn dash(platform: &str) -> String {
    platform.replacen('_', "-", 1)
}

fn gha_runs_on(build_platform: &str) -> Vec<String> {
    match build_platform {
        "osx_64" => vec!["macos-13".into()],
        "osx_arm64" => vec!["macos-14".into()],
        p if p.starts_with("win") => vec!["windows-latest".into()],
        _ => vec!["ubuntu-latest".into()],
    }
}

fn docker_image(target_platform: &str) -> Option<String> {
    let arch = target_platform.strip_prefix("linux_")?;
    let arch = match arch {
        "64" => "x86_64",
        other => other,
    };
    Some(format!("quay.io/condaforge/linux-anvil-{arch}"))
}

/// Compute the build matrix for a feedstock from its `conda-forge.yml`.
pub fn compute_matrix(feedstock: &Feedstock) -> Vec<BuildConfig> {
    let config = &feedstock.config;

    // Targets: the defaults plus anything mentioned in `provider:` or
    // `build_platform:`.
    let mut targets: Vec<String> = DEFAULT_TARGET_PLATFORMS
        .iter()
        .map(|s| s.to_string())
        .collect();
    for key in config.provider.keys().chain(config.build_platform.keys()) {
        if key.contains('_') && !targets.iter().any(|t| t == key) {
            targets.push(key.clone());
        }
    }

    // noarch recipes only build on the configured noarch platforms.
    let noarch = feedstock
        .recipe
        .parsed
        .as_ref()
        .and_then(|doc| lookup(doc, "build.noarch"))
        .is_some();
    if noarch {
        targets = if config.noarch_platforms.is_empty() {
            vec!["linux_64".to_string()]
        } else {
            config.noarch_platforms.clone()
        };
    }

    let used_vars = UsedVars::from_recipe(&feedstock.recipe);

    let mut matrix = Vec::new();
    for target in targets {
        let build_platform = config
            .build_platform
            .get(&target)
            .cloned()
            .unwrap_or_else(|| target.clone());
        let provider = match config.provider_for(&build_platform) {
            Provider::None => continue,
            Provider::Default => Provider::Azure,
            p => p,
        };

        // Fan out over the variant file, one job per matrix cell. A recipe
        // without a variant file gets exactly one (empty) cell. Keys the
        // recipe never references are pruned first, like conda-smithy.
        let mut variant_config =
            VariantConfig::from_recipe_dir(&feedstock.recipe_dir(), &target).unwrap_or_default();
        variant_config.prune(&used_vars);
        let fanout_keys = variant_config.fanout_keys();
        let cells = variant_config
            .expand()
            .unwrap_or_else(|_| vec![VariantCell::new()]);

        for cell in cells {
            let suffix = cell_name_suffix(&cell, &fanout_keys);
            let name = if suffix.is_empty() {
                target.clone()
            } else {
                format!("{target}_{suffix}")
            };
            matrix.push(BuildConfig {
                name,
                target_platform: dash(&target),
                build_platform: dash(&build_platform),
                os: os_of(&build_platform).to_string(),
                provider: provider.as_str().to_string(),
                upload: true,
                gha_runs_on: gha_runs_on(&build_platform),
                docker_image: docker_image(&target),
                cross_compile: build_platform != target,
                variant: cell,
            });
        }
    }
    matrix
}

/// The full template context. Everything a template needs, precomputed.
#[derive(Debug, Serialize)]
pub struct RenderContext {
    pub feedstock_name: String,
    pub package_name: String,
    /// All matrix entries.
    pub configs: Vec<BuildConfig>,
    /// Matrix entries per provider, for provider-specific templates.
    pub gha_configs: Vec<BuildConfig>,
    pub azure_configs: Vec<BuildConfig>,
    pub maintainers: Vec<String>,
    pub about: AboutContext,
    pub github_actions: GithubActionsContext,
    pub upload_on_branch: Option<String>,
    pub secrets: Vec<String>,
    pub build_tool: String,
    pub recipe_dir: String,
    /// The raw `conda-forge.yml`, for user templates that need custom keys.
    pub forge_config: serde_yaml::Value,
    /// smithy version that rendered this feedstock.
    pub smithy_version: String,
}

#[derive(Debug, Default, Serialize)]
pub struct AboutContext {
    pub home: Option<String>,
    pub summary: Option<String>,
    pub license: Option<String>,
    pub dev_url: Option<String>,
    pub doc_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GithubActionsContext {
    pub cancel_in_progress: bool,
    pub max_parallel: u32,
    pub timeout_minutes: u32,
    pub triggers: Vec<String>,
}

fn string_at(doc: Option<&serde_yaml::Value>, paths: &[&str]) -> Option<String> {
    let doc = doc?;
    paths
        .iter()
        .find_map(|p| lookup(doc, p))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

pub fn build_context(feedstock: &Feedstock) -> RenderContext {
    let configs = compute_matrix(feedstock);
    let doc = feedstock.recipe.parsed.as_ref();

    let maintainers = doc
        .and_then(|d| lookup(d, "extra.recipe-maintainers"))
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let gha = &feedstock.config.github_actions;
    RenderContext {
        feedstock_name: feedstock.name(),
        package_name: feedstock
            .recipe
            .package_name()
            .unwrap_or_else(|| feedstock.name()),
        gha_configs: configs
            .iter()
            .filter(|c| c.provider == "github_actions")
            .cloned()
            .collect(),
        azure_configs: configs
            .iter()
            .filter(|c| c.provider == "azure")
            .cloned()
            .collect(),
        configs,
        maintainers,
        about: AboutContext {
            home: string_at(doc, &["about.home", "about.homepage"]),
            summary: string_at(doc, &["about.summary"]),
            license: string_at(doc, &["about.license"]),
            dev_url: string_at(doc, &["about.dev_url", "about.repository"]),
            doc_url: string_at(doc, &["about.doc_url", "about.documentation"]),
        },
        github_actions: GithubActionsContext {
            cancel_in_progress: gha.cancel_in_progress,
            max_parallel: gha.max_parallel.unwrap_or(50),
            timeout_minutes: gha.timeout_minutes.unwrap_or(360),
            triggers: vec!["push".to_string(), "pull_request".to_string()],
        },
        upload_on_branch: feedstock.config.upload_on_branch.clone(),
        secrets: vec!["BINSTAR_TOKEN".to_string()],
        build_tool: feedstock
            .config
            .conda_build_tool
            .clone()
            .unwrap_or_else(|| match feedstock.recipe.version {
                crate::recipe::RecipeVersion::V1 => "rattler-build".to_string(),
                crate::recipe::RecipeVersion::V0 => "conda-build".to_string(),
            }),
        recipe_dir: feedstock.config.recipe_dir().to_string(),
        forge_config: feedstock.config.raw.clone(),
        smithy_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// A rendered output file (relative path + contents).
#[derive(Debug)]
pub struct RenderedFile {
    pub path: PathBuf,
    pub contents: String,
}

/// Collect the set of templates to render: built-ins (possibly overridden
/// by the user) plus extra user templates.
fn collect_templates(feedstock: &Feedstock) -> Result<BTreeMap<String, (PathBuf, String)>> {
    let mut templates = BTreeMap::new();
    let user_dir = feedstock.user_templates_dir();

    for (name, output) in BUILTIN_TEMPLATES {
        let user_path = user_dir.join(name);
        let source = if user_path.is_file() {
            fs_err::read_to_string(&user_path)?
        } else {
            builtin_template_source(name)
                .expect("BUILTIN_TEMPLATES entry without embedded source")
                .to_string()
        };
        templates.insert(name.to_string(), (PathBuf::from(output), source));
    }

    // Extra user templates: `<path with __ for />.j2`.
    if user_dir.is_dir() {
        let mut extra: Vec<_> = fs_err::read_dir(&user_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "j2"))
            .collect();
        extra.sort();
        for path in extra {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if templates.contains_key(&name) {
                continue; // an override of a built-in, already handled
            }
            let output = name.trim_end_matches(".j2").replace("__", "/");
            let source = fs_err::read_to_string(&path)?;
            templates.insert(name, (PathBuf::from(output), source));
        }
    }
    Ok(templates)
}

/// Render all templates for a feedstock. Returns the rendered files
/// (without writing them — the caller decides).
pub fn render_feedstock(feedstock: &Feedstock) -> Result<Vec<RenderedFile>> {
    let context = build_context(feedstock);
    let templates = collect_templates(feedstock)?;

    let mut env = Environment::new();
    env.set_keep_trailing_newline(true);
    // Templates produce YAML/Markdown, never HTML/JSON — don't let the
    // `.yml` template names switch minijinja into JSON auto-escaping.
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    for (name, (_, source)) in &templates {
        env.add_template_owned(name.clone(), source.clone())
            .with_context(|| format!("template `{name}` failed to parse"))?;
    }

    let ctx = minijinja::Value::from_serialize(&context);
    let mut rendered = Vec::new();

    for (name, (output, _)) in &templates {
        // Skip provider templates that would render for zero jobs.
        if name == "github-actions.yml.j2" && context.gha_configs.is_empty() {
            continue;
        }
        if name == "azure-pipelines.yml.j2" && context.azure_configs.is_empty() {
            continue;
        }
        let template = env.get_template(name)?;
        let contents = template
            .render(&ctx)
            .with_context(|| format!("template `{name}` failed to render"))?;
        rendered.push(RenderedFile {
            path: output.clone(),
            contents,
        });
    }

    // `.ci_support/<config>.yaml`, one per matrix entry (plain YAML dump,
    // no template needed).
    for config in &context.configs {
        let mut doc = serde_yaml::Mapping::new();
        doc.insert(
            "target_platform".into(),
            serde_yaml::Value::String(config.target_platform.clone()),
        );
        doc.insert(
            "build_platform".into(),
            serde_yaml::Value::String(config.build_platform.clone()),
        );
        if let Some(image) = &config.docker_image {
            doc.insert(
                "docker_image".into(),
                serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(image.clone())]),
            );
        }
        for (key, value) in &config.variant {
            doc.insert(
                serde_yaml::Value::String(key.clone()),
                serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(value.clone())]),
            );
        }
        rendered.push(RenderedFile {
            path: PathBuf::from(format!(".ci_support/{}.yaml", config.name)),
            contents: format!(
                "# This file was generated automatically by smithy. Do not edit.\n{}",
                serde_yaml::to_string(&serde_yaml::Value::Mapping(doc))?
            ),
        });
    }

    Ok(rendered)
}

/// Render and write everything into the feedstock. Returns the relative
/// paths that were written, honouring `skip_render` from `conda-forge.yml`.
pub fn rerender(feedstock: &Feedstock) -> Result<Vec<PathBuf>> {
    let files = render_feedstock(feedstock)?;
    let mut written = Vec::new();
    for file in files {
        let rel = file.path.to_string_lossy();
        if feedstock
            .config
            .skip_render
            .iter()
            .any(|skip| skip == rel.as_ref())
        {
            continue;
        }
        let absolute = feedstock.root.join(&file.path);
        if let Some(parent) = absolute.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(&absolute, file.contents)?;
        written.push(file.path);
    }
    Ok(written)
}
