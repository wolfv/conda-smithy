# GitHub Action

The repository ships a composite action at `rust/action` that builds
smithy (with cargo caching) and runs it against a feedstock.

## Lint on every PR

```yaml title=".github/workflows/lint.yml"
name: lint
on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: wolfv/conda-smithy/rust/action@master
        with:
          command: lint
          feedstock-dir: .
```

The action:

* fails the job when any lint (error) fires — hints don't fail it,
* writes a Markdown report to the job's **step summary**,
* exposes the JSON report as the `report` output for downstream steps:

```yaml
      - uses: wolfv/conda-smithy/rust/action@master
        id: smithy
        with:
          command: lint
      - name: Count hints
        run: echo '${{ steps.smithy.outputs.report }}' | jq '.hints | length'
```

## Check that a feedstock is rerendered

```yaml
      - uses: wolfv/conda-smithy/rust/action@master
        with:
          command: rerender-check
```

`rerender-check` prints the files a rerender would write; combine with a
follow-up `git diff --exit-code` after a full `rerender` to enforce that
committed CI files are up to date:

```yaml
      - uses: wolfv/conda-smithy/rust/action@master
        with:
          command: rerender
      - run: git diff --exit-code
```

## Inputs & outputs

| input | default | meaning |
|-------|---------|---------|
| `command` | `lint` | `lint`, `rerender-check` or `rerender` |
| `feedstock-dir` | `.` | path to the feedstock in the workspace |

| output | meaning |
|--------|---------|
| `report` | for `lint`: the JSON report (`{"lints": [...], "hints": [...]}`) |
