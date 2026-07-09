// conda-smithy R-019: url sources need a checksum.

if recipe_parse_error != "" { return; }

let sources = get(recipe, "source");
if type_of(sources) == "map" { sources = [sources]; }
if type_of(sources) != "array" { return; }

for src in sources {
    if type_of(src) != "map" || !has(src, "url") { continue; }
    if !has(src, "sha256") && !has(src, "sha1") && !has(src, "md5") {
        lint("When defining a source/url please add a sha256, sha1 or md5 checksum (sha256 preferably).");
    }
}
