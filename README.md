# docket

The records an agent works from: a backlog (open defects and postponed work, each with a trigger, a state and a
deadline), specs, and a log. `docket` checks that they keep their rules and writes their index files, so that whoever
picks up the work next, an LLM or a person, reads one index instead of every file.

The records live in `docs/` and are written in [OKF 0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format),
so any OKF tool can read them.

## Install

There is no release yet. To try docket now, build it from source with [rustup](https://rustup.rs/) installed (the
toolchain version comes from `rust-toolchain.toml`):

```sh
cargo build --release   # the binary is target/release/docket (docket.exe on Windows)
```

## Usage

```sh
docket --root <repository> check   # check the records; exits 1 when a rule is broken
docket --root <repository> index   # write every index.md from the frontmatter
```

`--root` defaults to the current directory. The records are read from `<repository>/docs`.

The index files only help if your agent reads them. Point it at them in your `AGENTS.md` (or whatever file your agent
reads first), for example:

```markdown
- **Before starting work, read `docs/backlog/index.md` and `docs/specs/index.md`, and open only the items and specs
  that concern the work.** Read `docs/done/index.md` and `docs/log.md` when you need to know why something was
  decided.
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
- **Every spec keeps the format:** `title`, `description` and `status`, a status that matches its directory, and a
  non-empty `# Resolution` once it is closed.
- **The log exists, points only at backlog items that exist,** and its second-level headings are dates, newest
  first.
- **Every document is a known type in its directory.**
- **Every index file equals what `docket index` writes.** Nobody maintains a list by hand.
- **No spec sits at the repository root.**

The frontmatter is read as YAML 1.2: quoting a value never changes whether it passes.

## Development

`cargo xtask ci` runs format, lint and tests, the same checks as CI. On Windows the host needs Visual Studio's C++
tools and the Windows SDK. Everything also runs in the container (`compose.yaml`), where the host needs only Docker:
`docker compose run --rm dev cargo xtask ci`.

The pip wheels (Windows x86_64 and Linux x86_64) are built with maturin, in the container:

```sh
docker compose run --rm dev maturin build --release --out dist                                   # Linux
docker compose run --rm dev maturin build --release --target x86_64-pc-windows-msvc --out dist  # Windows
```

The Windows wheel is cross-compiled with cargo-xwin, which downloads the MSVC runtime and the Windows SDK under
Microsoft's license. CI builds and tests it natively on Windows. Other platforms (macOS, Linux aarch64) are welcome as
contributions.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
