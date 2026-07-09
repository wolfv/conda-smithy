// conda-smithy RC-002 + macOS deployment-target checks (RC-000/CBC-001,
// simplified): sanity-check conda_build_config.yaml / variants.yaml.
//
// The engine puts the variant file's name and raw text into
// `variant_config_filename` / `variant_config_text`; `parse_yaml()` turns
// any YAML text into a scriptable value.

// RC-002: only one variant file may exist.
if "conda_build_config.yaml" in recipe_files && "variants.yaml" in recipe_files {
    lint("Found both conda_build_config.yaml and variants.yaml in the recipe directory; remove one of them.");
}

if variant_config_text == "" { return; }

// Strip `# [selector]` comments so the YAML parses, keeping every line.
let cleaned = "";
for line in variant_config_text.split("\n") {
    let idx = line.index_of("#");
    if idx >= 0 && line.contains("[") {
        cleaned += line.sub_string(0, idx);
    } else {
        cleaned += line;
    }
    cleaned += "\n";
}
let doc = parse_yaml(cleaned);
if type_of(doc) != "map" { return; }

fn ver_lt(a, b) {
    let pa = `${a}`.split(".");
    let pb = `${b}`.split(".");
    let n = if pa.len() < pb.len() { pa.len() } else { pb.len() };
    for i in 0..n {
        // Non-numeric components make the comparison inconclusive.
        if !is_match(pa[i], "^[0-9]+$") || !is_match(pb[i], "^[0-9]+$") {
            return false;
        }
        let x = pa[i].parse_int();
        let y = pb[i].parse_int();
        if x < y { return true; }
        if x > y { return false; }
    }
    pa.len() < pb.len()
}

fn first_of(doc, key) {
    let v = get(doc, key);
    if type_of(v) == "array" && v.len() > 0 { return v[0]; }
    if type_of(v) == "string" { return v; }
    ()
}

let deployment = first_of(doc, "MACOSX_DEPLOYMENT_TARGET");
let sdk = first_of(doc, "MACOSX_SDK_VERSION");
let stdlib_version = first_of(doc, "c_stdlib_version");

// The SDK must be at least as new as the deployment target.
if deployment != () && sdk != () && ver_lt(sdk, deployment) {
    lint(`MACOSX_SDK_VERSION (${sdk}) is lower than MACOSX_DEPLOYMENT_TARGET (${deployment}); the SDK must be at least as new as the deployment target.`);
}

// Deployment targets are driven through c_stdlib_version these days.
if deployment != () && stdlib_version == () {
    hint("You are setting MACOSX_DEPLOYMENT_TARGET without c_stdlib_version; prefer setting c_stdlib_version (it feeds the deployment target).");
}
