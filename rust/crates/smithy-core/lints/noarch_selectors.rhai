// conda-smithy R0-004 + R1-002 (simplified): noarch packages are built
// once, so per-platform skips or requirement selectors contradict them.
// If the feedstock declares `noarch_platforms` in conda-forge.yml,
// platform-only selectors on host/run requirements are allowed.

if recipe_parse_error != "" { return; }
if get(recipe, "build.noarch") == () { return; }

let noarch_platforms = get(config, "noarch_platforms");
let multiple_platforms = type_of(noarch_platforms) == "array" && noarch_platforms.len() > 1;

// A selector expression that only mentions OS families.
fn platform_only(expr) {
    is_match(expr, "^[\\s()]*(not\\s+|and\\s+|or\\s+|win|linux|osx|unix|[\\s()])+$")
}

if recipe_version == 1 {
    // R1-002: any `skip` under build contradicts noarch...
    if has(recipe, "build.skip") {
        lint("`noarch` packages can't have skips with selectors. If the selectors are necessary, please remove `noarch`.");
    }
    // ...and conditional requirements (if/then maps) only pass when they
    // are platform-only and noarch_platforms is configured.
    for section in ["build", "host", "run"] {
        let reqs = get(recipe, "requirements." + section);
        if type_of(reqs) != "array" { continue; }
        for req in reqs {
            if type_of(req) != "map" { continue; }
            let condition = get(req, "if");
            let allowed = multiple_platforms
                && (section == "host" || section == "run")
                && type_of(condition) == "string"
                && platform_only(condition);
            if !allowed {
                lint("`noarch` packages can't have selectors. If the selectors are necessary, please remove `noarch`.");
            }
        }
    }
} else {
    // v0: scan the raw text; track whether we're inside requirements'
    // host:/run: blocks.
    let in_requirements = false;
    let in_host_run = false;
    let line_no = 0;
    for line in recipe_text.split("\n") {
        line_no += 1;
        if is_match(line, "^requirements:") {
            in_requirements = true;
            in_host_run = false;
        } else if is_match(line, "^\\S") {
            in_requirements = false;
            in_host_run = false;
        } else if in_requirements && is_match(line, "^\\s+(host|run):") {
            in_host_run = true;
        } else if in_requirements && is_match(line, "^\\s+(build|run_constrained):") {
            in_host_run = false;
        }

        if !line.contains("# [") && !line.contains("#[") { continue; }

        if is_match(line, "^\\s*skip:") {
            lint(`noarch packages can't have skips with selectors (line ${line_no}). If the selectors are necessary, please remove noarch.`);
        } else if in_host_run {
            let selectors = find_all(line, "\\[[^\\]]+\\]");
            let allowed = false;
            if multiple_platforms && selectors.len() > 0 {
                let expr = selectors[0].sub_string(1, selectors[0].len() - 2);
                allowed = platform_only(expr);
            }
            if !allowed {
                let shown = line;
                shown.trim();
                lint(`noarch packages can't have selectors on requirements (line ${line_no}: ${shown}). If the selectors are necessary, please remove noarch.`);
            }
        }
    }
}
