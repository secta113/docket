# docket

docket keeps a project's structure from drifting while LLMs and people change it. It keeps two structures:

- **The layers:** which part of the code may import which (`handler`, `application`, `domain`, `infrastructure`,
  `utils`, and an optional `ui`). Code that cannot be split into these layers mixes responsibilities, so the layers
  are how an LLM, or a person, is made to split it: every piece of code has to land in a layer whose role and allowed
  imports are written down. docket makes them when a project starts and checks them on every run, so the direction
  of dependencies stays what it was meant to be.
- **The specs and records:** what is open, what is agreed, what is finished and why. A backlog (open defects and
  postponed work, each with a trigger, a state and a deadline), specs, and a log, each kept to strict rules.

## Principles

- **The declaration is the truth, and docket does not repair.** The structure a project declares is the structure. A
  tree that differs from it fails, either way, and someone decides whether the tree or the declaration is wrong;
  docket changes the tree only when asked.
- **Every rule has a check that can fail.** A rule without a check is only a label, and soon drifts.
- **A check with nothing to check fails.** A check that passes on an empty tree protects nothing.
- **The rules come with the tool.** A project pins one version of docket, and takes improvements to the rules by
  upgrading it, in a commit of its own.
- **The same structure in every language.** docket is one binary with no language runtime, so a Rust or TypeScript
  project keeps the same layers and records as a Python one.
- **Whoever picks up the work next reads one index, not every file.** The index files are generated, never written by
  hand.

docket checks the records today. Making and checking the layers is not built yet.

The records live in `docs/`, which is a bundle in [OKF 0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format)
(its `SPEC.md` as of commit `ad30107`): every document has YAML frontmatter with a `type`, `index.md` and `log.md` are
reserved names, and the log's headings are dates. An OKF reader can read the records as a bundle. The rules on top of
that format (a backlog item's trigger, state and deadline, a spec's status and directory, the shape of a log entry)
are docket's own, stricter than OKF, and not part of it. docket is not an OKF validator.

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
- **Every backlog item keeps the format:** the fields above with their types, and non-empty Trigger, State and Details
  (and Resolution when closed). A field docket does not know passes as an extension, as OKF allows, unless it looks
  like a misspelling of a field the document type has (`stale_afer`, `staleAfter`, `Title`), or is a backlog item's
  field on another type: those fail, in every document type, as a misspelled optional field would otherwise be
  silently dropped. A deadline is an event or a reason, never only a date
  (`2026-10-31`, `2026/10/31`, `31.10.2026`, `2026年10月31日` or a month alone); an event may contain a date. Every
  time has a time zone. `stale_after` is later than the last `verified`. A required text is not blank (spaces alone
  are empty), in every document type, and no tag is empty.
- **Every link in `# Details` resolves,** with heading anchors computed as GitHub computes them. A path with a drive
  letter (`C:/...`) fails: it names a file on one machine. Only a URL (`https:`, `mailto:`) is not checked, and a
  file name with a line number (`check.rs:104`) is a path, not a URL. A path is separated with `/`: only Windows
  reads a backslash as a separator, so a path with one fails. A link to a `.py` file names, in its text, a function
  or class defined there, at any depth. The file is parsed as Python, so a `def` line inside a string or a docstring
  does not count. The text is one name exactly as defined (``[`render_index`](../tests/bundle.py)``), not a call
  (`render_index()`) or a dotted path (`Bundle.render`); when it names nothing, the message says which name to write
  or lists the names the file defines.
- **Every spec keeps the format:** `title`, `description` and `status`, a status that matches its directory, and a
  non-empty `# Resolution` once it is closed.
- **The log exists, points only at backlog items that exist** (`backlog/<slug>.md`, written with `/` or `\`, the
  slug percent-encoded or not), and its second-level headings are dates, newest first. Below the title, the log is
  a flat list of entries grouped under the dates (OKF 0.2, section 9): every entry is a list item, and its indented
  lines (wrapped text, nested items) belong to it. A task heading (`### ...`), a paragraph, or an entry before the
  first date fails. A new log, with only its title and HTML comments, passes.
- **Every document is a known type in its directory.** The fields OKF defines for a document pass as OKF writes them
  (`generated` needs only `by`; every entry of `sources` needs a `resource`; `usage_window` is a `{from, to}` range).
  The names OKF reserves appear only where docket writes and reads them: `index.md` in `docs/` and in each directory
  of documents, `log.md` in `docs/`. A markdown file is named `.md`, in lowercase: GitHub shows a `.MD` file, but
  docket would not read it.
- **Every index file equals what `docket index` writes.** Nobody maintains a list by hand.
- **No spec sits at the repository root.**

The frontmatter is read as YAML 1.2: quoting a value never changes whether it passes. Its closing `---` may end the
file. The body is read as GitHub
renders it: an HTML comment or a fenced code block (``` or ~~~, indented by up to 3 spaces, running to the end when
it is not closed) is not text, so a heading, a link or the only text of a section inside one counts for nothing. A
heading may be indented by up to 3 spaces, and a closing run of `#` (`## Notes ##`) is not part of it.

## Development

`cargo xtask ci` runs format, lint and tests, the same checks as CI. It also fails when the map in `AGENTS.md`
misses a tracked top-level path or module of `src/` (or names one that is gone), and when `rust-toolchain.toml`,
the `Dockerfile` and the CI workflow name different toolchain versions. On Windows the host needs Visual Studio's C++
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
