// A custom lint rule, written by a feedstock maintainer — no Rust, no
// recompilation. Drop any *.rhai file into .smithy/lints/ and it runs
// alongside the built-in rules on the next `smithy lint`.
//
// This one flags placeholder URLs that were never replaced.

let home = get(recipe, "about.homepage");
if type_of(home) == "string" && home.contains("example.com") {
    hint("about.homepage still points at example.com — did you forget to set the real project URL?");
}

for src in find_all(recipe_text, "https?://example\\.com[^\\s\"']*") {
    hint(`source URL ${src} looks like a placeholder.`);
}
