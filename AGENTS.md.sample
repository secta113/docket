# AGENTS.md (docket)

Rules shared by all repositories are in `~/.agents/AGENTS.md` (secta113/agent-guidelines). This file holds only
**the map of this repository**, **where it differs from the shared defaults**, and **short rules that apply to every
change**. Where the two disagree, this file wins.

## Map

| Path | Content |
|---|---|
| `src/` | The tool: checks the records an agent works from (backlog, specs, log in `docs/`, written in OKF 0.2) and writes their index files |
| `tests/` | Tests that run the built binary as a user runs it |
| `xtask/` | The CI entry point (`cargo xtask ci`) |
| `rust-toolchain.toml` | The one place that names the toolchain. The Dockerfile and CI install from it |
| `pyproject.toml` | The pip package: a wheel that carries only the binary (maturin, `bindings = "bin"`) |
| `requirements-build.txt` | The one place that names the packaging tools (maturin, cargo-xwin). The Dockerfile and CI install from it |

## Differences from the shared defaults

- **Not made from project-template.** The template is for Python projects. This tool is a port of the template's
  record checks (`tests/backlog_bundle.py` and its siblings), and its behaviour is compared with them.
- **No layers yet.** The layers are a Python layout, and the tool is small. Revisit when `src/` has more than a few
  modules.
- **No records (`docs/`) yet.** They come when this tool can check them. Until then, the work is recorded in
  project-template (`docs/backlog/okf-backlog-rust-tool.md` and its log).

## Rules for every change

- **Run CI with `cargo xtask ci`, on the host or in the container (`docker compose run --rm dev cargo xtask ci`).** The
  container is the Linux of CI and needs only Docker. On Windows, the host needs Visual Studio's C++ tools and the
  Windows SDK; without the SDK, linking fails (`kernel32.lib` not found), and Git Bash's own `link` gets in the way.
- **Change the toolchain version only in `rust-toolchain.toml`,** and the base image (`Dockerfile`, the `container`
  in `.github/workflows/ci.yml`) to the same version. Change the packaging tools only in `requirements-build.txt`.
- **Try a wheel as a user gets it:** build it (README) and `pip install` it into a fresh venv, outside the build tree.
