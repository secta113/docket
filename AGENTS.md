# AGENTS.md (docket)

This file holds **the map of this repository** and **short rules that apply to every change**. What docket checks,
and why, is in `README.md`.

## Map

| Path | Content |
|---|---|
| `src/` | The tool, one module per concern (below) |
| `tests/` | Tests that run the built binary as a user runs it (`cli.rs`) |
| `xtask/` | The CI entry point (`cargo xtask ci`) |
| `.cargo/` | The `cargo xtask` alias |
| `.github/` | GitHub Actions: `ci.yml` runs `cargo xtask ci` in the container, and builds and installs the wheels on Linux and Windows |
| `Cargo.toml` | The package, and the one place that names docket's version (maturin takes the wheel version from it) |
| `rust-toolchain.toml` | The one place that names the toolchain. The Dockerfile and CI install from it |
| `Dockerfile`, `compose.yaml` | The development container: the Linux of CI, with the packaging tools |
| `pyproject.toml` | The pip package: a wheel that carries only the binary (maturin, `bindings = "bin"`) |
| `requirements-build.txt` | The one place that names the packaging tools (maturin, cargo-xwin). The Dockerfile and CI install from it |
| `README.md` | What docket checks, how to use it, and how to build it |
| `LICENSE-MIT`, `LICENSE-APACHE` | The license: MIT OR Apache-2.0 |

Also tracked, as in most repositories: `.gitattributes`, `.gitignore`, `Cargo.lock`, `AGENTS.md`, and `CLAUDE.md`
(which only imports `AGENTS.md`).

| Module of `src/` | Content |
|---|---|
| `main.rs` | The command line: `docket check` and `docket index` |
| `lib.rs` | The library the command line calls |
| `check.rs` | Every rule `docket check` runs, each with its floor |
| `bundle.rs` | Reading `docs/` as one OKF bundle, and rendering the index files |
| `schema.rs` | The frontmatter of each document type |
| `frontmatter.rs` | Splitting a document into frontmatter and the sections of its body |
| `markdown.rs` | What GitHub renders as text, headings and their anchors as GitHub computes them, and links |
| `source.rs` | Reading files, and the paths in messages |

## How this repository differs from what docket keeps

- **No `docs/` here.** docket's own backlog, specs and log are kept outside this repository.
- **No layers.** The modules are split by concern, not into `handler`, `application`, `domain` and the other layers
  that docket is meant to keep in the projects that use it (README).

## Rules for every change

- **Run CI with `cargo xtask ci`, on the host or in the container (`docker compose run --rm dev cargo xtask ci`).** The
  container is the Linux of CI and needs only Docker. On Windows, the host needs Visual Studio's C++ tools and the
  Windows SDK; without the SDK, linking fails (`kernel32.lib` not found), and Git Bash's own `link` gets in the way.
- **Change the toolchain version only in `rust-toolchain.toml`,** and the base image (`Dockerfile`, the `container`
  in `.github/workflows/ci.yml`) to the same version. Change the packaging tools only in `requirements-build.txt`.
- **Try a wheel as a user gets it:** build it (README) and `pip install` it into a fresh venv, outside the build tree.
- **A change that can fail records that passed before (a new rule, a stricter rule) raises the minor version** while
  docket is `0.x`. Projects pin the exact version, so they take the change in a commit of their own.
