//! End-to-end smoke tests for the `smithy` binary.

use std::path::PathBuf;
use std::process::Command;

fn smithy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_smithy"))
}

fn demo_feedstock() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo-feedstock")
}

#[test]
fn lint_demo_feedstock_succeeds() {
    let output = smithy()
        .args(["lint", "--feedstock-dir"])
        .arg(demo_feedstock())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    // The demo's custom rule produces hints, never lints.
    assert!(stdout.contains("no_example_urls"), "stdout: {stdout}");
    assert!(stdout.contains("0 lint(s)"), "stdout: {stdout}");
}

#[test]
fn lint_fails_on_bad_recipe() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("recipe")).unwrap();
    std::fs::write(dir.path().join("recipe/meta.yaml"), "package:\n  name: x\n").unwrap();

    let output = smithy()
        .args(["lint", "--feedstock-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("build/number"), "stdout: {stdout}");
}

#[test]
fn rerender_check_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("recipe")).unwrap();
    std::fs::write(
        dir.path().join("recipe/recipe.yaml"),
        "package:\n  name: x\n  version: \"1\"\n",
    )
    .unwrap();

    let output = smithy()
        .args(["rerender", "--check", "--feedstock-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would write README.md"), "stdout: {stdout}");
    assert!(!dir.path().join("README.md").exists());
}

#[test]
fn lint_json_output() {
    let output = smithy()
        .args(["lint", "--format", "json", "--feedstock-dir"])
        .arg(demo_feedstock())
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert!(payload["lints"].as_array().unwrap().is_empty());
    assert!(payload["hints"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["rule"] == "no_example_urls"));
}

#[test]
fn init_creates_lintable_rerenderable_feedstock() {
    let dir = tempfile::tempdir().unwrap();

    let output = smithy()
        .args(["init", "widgets", "--feedstock-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(dir.path().join("recipe/recipe.yaml").exists());
    assert!(dir.path().join("conda-forge.yml").exists());
    assert!(dir.path().join(".smithy/lints/example.rhai").exists());

    // The skeleton must lint clean and rerender out of the box.
    let lint = smithy()
        .args(["lint", "--feedstock-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(
        lint.status.success(),
        "{}",
        String::from_utf8_lossy(&lint.stdout)
    );

    let rerender = smithy()
        .args(["rerender", "--feedstock-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(rerender.status.success());
    assert!(dir
        .path()
        .join(".github/workflows/conda-build.yml")
        .exists());

    // Re-running init must not clobber anything.
    let again = smithy()
        .args(["init", "widgets", "--feedstock-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&again.stdout);
    assert!(stdout.contains("nothing to do"), "{stdout}");
}
