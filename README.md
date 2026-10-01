# docket

The records an agent works from: a backlog (open defects and postponed work, each with a trigger, a state and a
deadline), specs, and a log. `docket` checks that they keep their rules and writes their index files, so that whoever
picks up the work next, an LLM or a person, reads one index instead of every file.

The records live in `docs/` and are written in [OKF 0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format),
so any OKF tool can read them. A port of the record checks in
[project-template](https://github.com/secta113/project-template), so that a project not written in Python does not
need Python only for its records.

Work in progress: ported from project-template, not yet packaged for pip.

```sh
docket --root <repository> check   # check the records
docket --root <repository> index   # write every index.md from the frontmatter
```

## Development

```sh
docker compose run --rm dev cargo xtask ci
```
