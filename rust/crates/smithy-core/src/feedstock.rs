//! A feedstock directory: `conda-forge.yml` + recipe + user customisations.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::ForgeConfig;
use crate::recipe::Recipe;

/// Directory (relative to the feedstock root) holding user extensions:
/// `.smithy/lints/*.rhai` and `.smithy/templates/*`.
pub const SMITHY_DIR: &str = ".smithy";

#[derive(Debug)]
pub struct Feedstock {
    pub root: PathBuf,
    pub config: ForgeConfig,
    pub recipe: Recipe,
}

impl Feedstock {
    pub fn load(root: &Path) -> Result<Self> {
        let config = ForgeConfig::from_feedstock_dir(root)?;
        let recipe_dir = root.join(config.recipe_dir());
        let recipe = Recipe::from_recipe_dir(&recipe_dir)?;
        Ok(Feedstock {
            root: root.to_path_buf(),
            config,
            recipe,
        })
    }

    /// Name of the feedstock: `extra.feedstock-name` from the recipe, the
    /// package name, or the directory name (sans `-feedstock`).
    pub fn name(&self) -> String {
        if let Some(doc) = &self.recipe.parsed {
            if let Some(name) =
                crate::recipe::lookup(doc, "extra.feedstock-name").and_then(|v| v.as_str())
            {
                return name.to_string();
            }
        }
        if let Some(name) = self.recipe.package_name() {
            return name;
        }
        self.root
            .file_name()
            .map(|n| {
                n.to_string_lossy()
                    .trim_end_matches("-feedstock")
                    .to_string()
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn recipe_dir(&self) -> PathBuf {
        self.root.join(self.config.recipe_dir())
    }

    pub fn smithy_dir(&self) -> PathBuf {
        self.root.join(SMITHY_DIR)
    }

    /// Directory with user-supplied lint scripts.
    pub fn user_lints_dir(&self) -> PathBuf {
        self.smithy_dir().join("lints")
    }

    /// Directory with user-supplied template overrides.
    pub fn user_templates_dir(&self) -> PathBuf {
        self.smithy_dir().join("templates")
    }
}
