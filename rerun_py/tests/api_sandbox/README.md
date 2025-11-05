# API sandbox

Sandbox environment for:
- tested snippets using our existing API
- mocks of WIP future APIs (shimmed over the existing API)
- tested snippets using the future API

## Structure

The `rerun_draft` is a mock of the future API.

The tests in `test_current/` and `test_draft/` are intended to match such that one can directly diff the directories:

```
git diff --no-index rerun_py/tests/api_sandbox/test_current rerun_py/tests/api_sandbox/test_draft
```

Tests outside of these directories are just misc. for illustration purposes.