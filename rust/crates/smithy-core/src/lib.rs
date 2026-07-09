//! `smithy-core` — the engine behind the Rust rewrite of conda-smithy.
//!
//! The crate is organised around three ideas:
//!
//! 1. **Parsing** ([`config`], [`recipe`]): load `conda-forge.yml` and the
//!    recipe (both v0 `meta.yaml` and v1 `recipe.yaml`) into plain data.
//! 2. **Linting** ([`lint`]): lint rules are [Rhai](https://rhai.rs) scripts.
//!    The built-in rules ship embedded in the binary, and feedstock
//!    maintainers can drop extra `*.rhai` files into `.smithy/lints/` —
//!    no Rust knowledge required.
//! 3. **Rendering** ([`render`]): CI configuration is generated from
//!    [minijinja](https://docs.rs/minijinja) templates. Built-in templates
//!    can be overridden per feedstock via `.smithy/templates/`.

pub mod config;
pub mod feedstock;
pub mod lint;
pub mod recipe;
pub mod render;

pub use config::ForgeConfig;
pub use feedstock::Feedstock;
pub use lint::{LintReport, Severity};
pub use recipe::{Recipe, RecipeVersion};
