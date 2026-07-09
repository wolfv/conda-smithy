// conda-smithy R-033 + R-034 + R-035: modern stdlib handling.
//
// Note: in v0 recipes `{{ compiler('c') }}` has already been rendered to
// `c_compiler_stub` (and `{{ stdlib('c') }}` to `c_stdlib_stub`) before
// linting, so both spellings are matched below.

if recipe_parse_error != "" { return; }

let build = get(recipe, "requirements.build");
if type_of(build) == "array" {
    let has_c_compiler = false;
    let has_stdlib = false;

    for req in build {
        if type_of(req) != "string" { continue; }

        if is_match(req, "(m2w64_)?(c|cxx|fortran)_compiler_stub")
            || is_match(req, "\\$\\{\\{\\s*compiler\\([\"'](c|cxx|fortran)[\"']\\)") {
            has_c_compiler = true;
        }
        if req.contains("_stdlib_stub") || is_match(req, "\\$\\{\\{\\s*stdlib\\(") {
            has_stdlib = true;
        }
        // R-034: no direct sysroot pins.
        if is_match(req, "sysroot_linux") {
            lint("You're setting a requirement on sysroot_linux-<arch> directly; please use `{{ stdlib('c') }}` with `c_stdlib_version` instead.");
        }
    }

    // R-033: compilers require an accompanying stdlib dependency.
    if has_c_compiler && !has_stdlib {
        lint("This recipe is using a compiler, which now requires adding a build dependence on `{{ stdlib('c') }}` as well.");
    }
}

// R-035: no direct __osx constraints.
for section in ["run", "run_constrained", "run_constraints"] {
    let reqs = get(recipe, "requirements." + section);
    if type_of(reqs) != "array" { continue; }
    for req in reqs {
        if type_of(req) == "string" && req.starts_with("__osx >") {
            lint("You're setting a constraint on the `__osx` virtual package directly; please use `c_stdlib_version` instead.");
        }
    }
}
