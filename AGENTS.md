# AGENTS.md (docket)

This file holds **the map of this repository** and **short rules that apply to every change**. What docket checks,
and why, is in `README.md`.

## Map

| Path | Content |
|---|---|
| `src/` | The tool, one module per concern (below) |
| `layers/` | The layer definitions, built into the binary: `table.toml` (what the layers are, in every stack) and one layout per stack (`python.toml`, `typescript.toml`, `rust.toml`: where each layer lives and the files that make it) |
| `records/` | The records skeleton, built into the binary: `rules.md` (the backlog rules docket writes into every project) and `log.md` (the log `docket create` starts) |
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
| `main.rs` | The command line: `docket init`, `docket create`, `docket check` and `docket index` |
| `lib.rs` | The library the command line calls |
| `check.rs` | Every rule `docket check` runs, each with its floor |
| `layers.rs` | Reading the layer definitions in `layers/` and a project's `.config/docket.toml` |
| `structure.rs` | The structure check: the tree agrees with `.config/docket.toml`, either way |
| `init.rs` | `docket init`: writing a project's declaration, once |
| `create.rs` | `docket create`: making the layers and the records skeleton a project lacks |
| `bundle.rs` | Reading `docs/` as one OKF bundle, and the files docket generates in it (the index files and the backlog rules) |
| `schema.rs` | The frontmatter of each document type |
| `frontmatter.rs` | Splitting a document into frontmatter and the sections of its body |
| `markdown.rs` | What GitHub renders as text, headings and their anchors as GitHub computes them, and links |
| `source.rs` | Reading files, and the paths in messages |

## How this repository differs from what docket keeps

- **No `docs/` here.** docket's own backlog, specs and log are kept outside this repository.
- **No layers, and no `.config/docket.toml`.** The modules are split by concern, not into `handler`, `application`,
  `domain` and the other layers that docket keeps in the projects that use it (README).

## Rules for every change

- **A layer or a stack changes in `layers/`, not in code.** The table says what the layers are once; a layout says
  only where they live in one stack. The tests read every definition, so a layout that names a layer the table lacks
  fails.

- **Run CI with `cargo xtask ci`, on the host or in the container (`docker compose run --rm dev cargo xtask ci`).** The
  container is the Linux of CI and needs only Docker. On Windows, the host needs Visual Studio's C++ tools and the
  Windows SDK; without the SDK, linking fails (`kernel32.lib` not found), and Git Bash's own `link` gets in the way.
- **Change the toolchain version only in `rust-toolchain.toml`,** and the base image (`Dockerfile`, the `container`
  in `.github/workflows/ci.yml`) to the same version. Change the packaging tools only in `requirements-build.txt`.
- **`ruff_python_parser` and `ruff_python_ast` are pinned to one exact version, and move together.** They are
  internal crates of Ruff, whose API changes between any two versions. Bump them by hand when the toolchain changes
  (a new Ruff may need a newer Rust) and when Python gains syntax docket fails to read.
- **Try a wheel as a user gets it:** build it (README) and `pip install` it into a fresh venv, outside the build tree.
- **A change that can fail records that passed before (a new rule, a stricter rule) raises the minor version** while
  docket is `0.x`. Projects pin the exact version, so they take the change in a commit of their own.
