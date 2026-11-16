# Contributing

- CommonJS modules, exported with one `module.exports` line at the end of the file.
- No dependencies. Tests use `node:test` and live in `test/`, one file per module.
- Every change comes with its tests, and `node --test` passes before it is merged.
- Command-line options are long names only, such as `--name`.
- Output for people is plain text on stdout. Errors go to stderr, with exit code 1.
- Output as JSON: one object, its keys in camelCase.
- Commit messages say what changed, in one line.
