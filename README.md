# docket

The records an agent works from: a backlog (open defects and postponed work, each with a trigger, a state and a
deadline), specs, and a log. `docket` checks that they keep their rules and writes their index files, so that whoever
picks up the work next, an LLM or a person, reads one index instead of every file.

The records live in `docs/` and are written in [OKF 0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format),
so any OKF tool can read them. A port of the record checks in
[project-template](https://github.com/secta113/project-template), so that a project not written in Python does not
need Python only for its records.

Not on PyPI yet. Build the wheels locally (below), then `pip install dist/<wheel>`.

```sh
docket --root <repository> check   # check the records
docket --root <repository> index   # write every index.md from the frontmatter
```

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
