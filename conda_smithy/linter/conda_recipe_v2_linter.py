import os
import re
from typing import Any, Dict, List, Optional

from rattler_build_conda_compat.jinja.jinja import (
    RecipeWithContext,
    render_recipe_with_context,
)

from conda_smithy.linter.errors import HINT_NO_ARCH
from conda_smithy.linter.utils import (
    TEST_FILES,
    _lint_package_version,
    _lint_recipe_name,
)

REQUIREMENTS_ORDER = ["build", "host", "run"]

EXPECTED_SINGLE_OUTPUT_SECTION_ORDER = [
    "context",
    "package",
    "source",
    "build",
    "requirements",
    "tests",
    "about",
    "extra",
]

EXPECTED_MULTIPLE_OUTPUT_SECTION_ORDER = [
    "context",
    "recipe",
    "source",
    "build",
    "outputs",
    "about",
    "extra",
]
TEST_KEYS = {"script", "python"}
JINJA_VAR_PAT = re.compile(r"\${{(.*?)}}")


def lint_recipe_tests(
    recipe_dir: Optional[str],
    test_section: List[Dict[str, Any]],
    outputs_section: List[Dict[str, Any]],
    lints: List[str],
    hints: List[str],
):
    tests_lints = []
    tests_hints = []

    if not any(key in TEST_KEYS for key in test_section):
        a_test_file_exists = recipe_dir is not None and any(
            os.path.exists(os.path.join(recipe_dir, test_file))
            for test_file in TEST_FILES
        )
        if a_test_file_exists:
            return

        if not outputs_section:
            lints.append("The recipe must have some tests.")
        else:
            has_outputs_test = False
            no_test_hints = []
            for section in outputs_section:
                test_section = section.get("tests", {})
                if any(key in TEST_KEYS for key in test_section):
                    has_outputs_test = True
                else:
                    no_test_hints.append(
                        "It looks like the '{}' output doesn't "
                        "have any tests.".format(section.get("name", "???"))
                    )
            if has_outputs_test:
                hints.extend(no_test_hints)
            else:
                lints.append("The recipe must have some tests.")

    lints.extend(tests_lints)
    hints.extend(tests_hints)


def hint_noarch_usage(
    build_section: Dict[str, Any],
    requirement_section: Dict[str, Any],
    hints: List[str],
):
    build_reqs = requirement_section.get("build", None)
    if (
        build_reqs
        and not any(
            [
                b.startswith("${{")
                and ("compiler('c')" in b or 'compiler("c")' in b)
                for b in build_reqs
            ]
        )
        and ("pip" in build_reqs)
    ):
        no_arch_possible = True
        if "skip" in build_section:
            no_arch_possible = False

        for _, section_requirements in requirement_section.items():
            if any(
                isinstance(requirement, dict)
                for requirement in section_requirements
            ):
                no_arch_possible = False
                break

        if no_arch_possible:
            hints.append(HINT_NO_ARCH)


def get_recipe_name(recipe_content: RecipeWithContext) -> str:
    rendered_context_recipe = render_recipe_with_context(recipe_content)
    package_name = (
        rendered_context_recipe.get("package", {}).get("name", "").strip()
    )
    recipe_name = (
        rendered_context_recipe.get("recipe", {}).get("name", "").strip()
    )
    return package_name or recipe_name


def get_recipe_version(recipe_content: RecipeWithContext) -> str:
    rendered_context_recipe = render_recipe_with_context(recipe_content)
    package_version = (
        rendered_context_recipe.get("package", {}).get("version", "").strip()
    )
    recipe_version = (
        rendered_context_recipe.get("recipe", {}).get("version", "").strip()
    )
    return package_version or recipe_version


def lint_recipe_name(
    recipe_content: RecipeWithContext,
    lints: List[str],
) -> None:
    name = get_recipe_name(recipe_content)

    lint_msg = _lint_recipe_name(name)
    if lint_msg:
        lints.append(lint_msg)


def lint_package_version(
    recipe_content: RecipeWithContext,
    lints: List[str],
) -> None:
    version = get_recipe_version(recipe_content)

    lint_msg = _lint_package_version(version)

    if lint_msg:
        lints.append(lint_msg)


def lint_usage_of_selectors_for_noarch(
    noarch_value: str,
    requirements_section: Dict[str, Any],
    build_section: Dict[str, Any],
    noarch_platforms: bool,
    lints: List[str],
):
    for section in requirements_section:
        section_requirements = requirements_section[section]

        if not section_requirements:
            continue

        has_bad_selector = False

        if any(isinstance(req, dict) for req in section_requirements):
            if noarch_platforms and section in ("host", "run"):
                for req in section_requirements:
                    if isinstance(req, dict) and not has_bad_selector:
                        for key in req:
                            if key == "if":
                                if_selectors = {
                                    selector
                                    for selector in req[key].split()
                                    if selector not in ("not", "and", "or")
                                }
                                allowed_nouns = (
                                    {"win", "linux", "osx", "unix"}
                                    if noarch_platforms
                                    else set()
                                )
                                if not if_selectors.issubset(allowed_nouns):
                                    has_bad_selector = True
                                    break
            if not noarch_platforms:
                has_bad_selector = True

            if has_bad_selector:
                lints.append(
                    "`noarch` packages can't have selectors. If "
                    "the selectors are necessary, please remove "
                    f"`noarch: {noarch_value}`."
                )
                break

    if "skip" in build_section:
        lints.append(
            "`noarch` packages can't have skips with selectors. If "
            "the selectors are necessary, please remove "
            f"`noarch: {noarch_value}`."
        )


def lint_sources(sources_section: list[dict[str, Any]], lints: List[str]):
    for source in sources_section:
        if "url" in source:
            # make sure that we have a hash
            if not (source.keys() & {"sha256", "md5"}):
                lints.append("Source must have a sha256 or md5 checksum.")
            # make sure that the hash is not a template (${{ ... }} will not match)
            if source.get("sha256"):
                if not re.match(r"[0-9a-f]{64}", source["sha256"]):
                    lints.append(
                        "sha256 checksum must be 64 characters long. Templates are not allowed for the sha256 checksum."
                    )
            if source.get("md5"):
                if not re.match(r"[0-9a-f]{32}", source["md5"]):
                    lints.append(
                        "md5 checksum must be 32 characters long. Templates are not allowed for the md5 checksum."
                    )
