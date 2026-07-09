// conda-smithy R-007 + R-010 + R-013: license sanity checks.

if recipe_parse_error != "" { return; }

let license = get(recipe, "about.license");
if type_of(license) != "string" { return; }

let lower = license.to_lower();
lower.trim(); // note: Rhai's trim() mutates in place

// R-007: "unknown" is not a license.
if lower == "unknown" {
    lint("The recipe license cannot be unknown.");
}

// R-010: SPDX identifiers don't contain the word "License".
if lower.contains("license")
    && !lower.contains("unlicense")
    && !lower.contains("licenseref")
    && !lower.contains("-license") {
    lint("The recipe `license` should not include the word \"License\".");
}

// R-013: licenses that require distributing the license text need a
// license_file entry.
if !has(recipe, "about.license_file") {
    if recipe_version == 1 {
        lint("license_file entry is missing, but is required.");
    } else {
        let family = get(recipe, "about.license_family");
        let fam = if type_of(family) == "string" { family.to_lower() } else { lower };
        for needs_file in ["gpl", "bsd", "mit", "apache", "psf"] {
            if fam.contains(needs_file) {
                lint("license_file entry is missing, but is required.");
                break;
            }
        }
    }
}
