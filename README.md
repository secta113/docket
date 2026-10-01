# docket

The records an agent works from: a backlog (open defects and postponed work, each with a trigger, a state and a
deadline), specs, and a log. `docket` checks that they keep their rules and writes their index files, so that whoever
picks up the work next, an LLM or a person, reads one index instead of every file.

The records live in `docs/` and are written in [OKF 0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format),
so any OKF tool can read them. docket is one binary, so a project in any language can check its records without
installing a language runtime for it.

Not on PyPI yet. Build the wheels locally (below), then `pip install dist/<wheel>`.

```sh
docket --root <repository> check   # check the records
docket --root <repository> index   # write every index.md from the frontmatter
```

## The records

```
docs/
  index.md          generated
  log.md            what was done, newest first
  backlog/
    rules.md        the backlog rules (type: Guide)
    index.md        generated
    <slug>.md       one item per file (type: Backlog Item)
  specs/            specs in progress (type: Spec, status: draft or stable)
    index.md        generated
  done/             closed specs (type: Spec, status: deprecated)
    index.md        generated
```

A backlog item has this frontmatter and these body headings:

```markdown
---
type: Backlog Item
title: Some problem
description: One sentence: what the problem is.
tags: [area]                  # exactly one; the index groups items by it
status: stable                # stable = open, deprecated = closed
filed: 2026-10-01
verified: {by: human:someone, at: 2026-10-01T10:00:00+09:00}
deadline_kind: until          # until, or none with the reason in deadline
deadline: until the next deploy
stale_after: 2027-01-01T00:00:00+09:00   # optional: when to measure the state again
---

# Trigger
# State
# Details
# Resolution    (only when closed)
```

## What `docket check` checks

- **The bundle is there:** `docs/`, `docs/index.md`, `docs/backlog/`, `docs/backlog/rules.md`, `docs/specs/` and
  `docs/done/` exist. Without them every other check would pass with nothing checked.
- **Every backlog item keeps the format:** the fields above with their types, no unknown field, and non-empty
  Trigger, State and Details (and Resolution when closed). A deadline is an event or a reason, never only a date.
  Every time has a time zone. `stale_after` is later than the last `verified`.
- **Every link in `# Details` resolves,** with heading anchors computed as GitHub computes them.
- **The log points only at backlog items that exist,** and its second-level headings are dates, newest first.
- **Every document is a known type in its directory,** and a spec's status matches its directory.
- **Every index file equals what `docket index` writes.** Nobody maintains a list by hand.
- **No spec sits at the repository root.**

The frontmatter is read as YAML 1.2: quoting a value never changes whether it passes.

## Development

`cargo xtask ci` runs format, lint and tests on the host. On Windows that needs Visual Studio's C++ tools and the
Windows SDK. Everything also runs in the container (`compose.yaml`), where the host needs only Docker:

```sh
docker compose run --rm dev cargo xtask ci                                            # format, lint, tests
docker compose run --rm dev maturin build --release --out dist                        # Linux wheel
docker compose run --rm dev maturin build --release --target x86_64-pc-windows-msvc --out dist  # Windows wheel
```

The Windows wheel is cross-compiled with cargo-xwin, which downloads the MSVC runtime and the Windows SDK under
Microsoft's license. CI builds and tests it natively on Windows.

Wheels are built for Windows x86_64 and Linux x86_64 only. Other platforms (macOS, Linux aarch64) are welcome as
contributions.
