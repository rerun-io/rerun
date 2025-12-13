# P03: Subprocess Helpers

**Status:** IN_PROGRESS
**Goal:** Easy way to run external commands with good output handling.

## Overview

Task runners commonly need to shell out to external tools (cargo, uv, npm, etc.). This module provides ergonomic helpers that:
- Integrate cleanly with recompose's Result type
- Stream output in real-time OR capture it
- Work well with `recompose.out()` / `recompose.dbg()`
- Handle errors gracefully

## Design

### Core API

```python
import recompose
from recompose import run, RunResult

# Basic usage - runs command, streams output, returns result
result = recompose.run("cargo", "build", "--release")

# Result has exit code and captured output
if result.ok:
    print(f"Success! stdout: {result.stdout}")
else:
    print(f"Failed with code {result.returncode}: {result.stderr}")

# With working directory
result = recompose.run("ls", "-la", cwd="/tmp")

# Capture output instead of streaming (for parsing)
result = recompose.run("git", "status", "--porcelain", capture=True)

# Environment variables
result = recompose.run("cargo", "build", env={"RUSTFLAGS": "-D warnings"})

# Check mode - raises exception on non-zero exit (for use in tasks)
recompose.run("cargo", "fmt", "--check", check=True)  # Raises on failure
```

### RunResult Type

```python
@dataclass
class RunResult:
    """Result from running a subprocess."""
    returncode: int
    stdout: str
    stderr: str
    command: list[str]

    @property
    def ok(self) -> bool:
        return self.returncode == 0

    @property
    def failed(self) -> bool:
        return self.returncode != 0
```

### Integration with Tasks

```python
@recompose.task
def build_project() -> recompose.Result[str]:
    result = recompose.run("cargo", "build", "--release")
    if result.failed:
        return recompose.Err(f"Build failed: {result.stderr}")
    return recompose.Ok("Build succeeded")
```

## Implementation Steps

1. **Create `RunResult` dataclass** - Simple container for subprocess results
2. **Implement `run()` function** - Core subprocess wrapper
   - Accept *args for command
   - `cwd` parameter for working directory
   - `env` parameter for environment variables (merged with os.environ)
   - `capture` parameter to switch between streaming and capturing
   - `check` parameter to raise on non-zero exit
3. **Output integration** - Stream output through `recompose.out()` when not capturing
4. **Error handling** - Convert subprocess errors to clean messages
5. **Tests** - Cover basic usage, error cases, output capture, env vars

## Key Decisions

1. **Use `*args` for command** - `run("cargo", "build")` is cleaner than `run(["cargo", "build"])`
2. **Stream by default** - Most task output should be visible in real-time
3. **Merge env vars** - Don't replace entire environment, just add/override
4. **No shell=True** - Security risk, explicit command parsing is safer

## Completion Criteria

- [ ] `recompose.run()` function works with basic commands
- [ ] Output streams to console in real-time (default behavior)
- [ ] `capture=True` captures output for parsing
- [ ] `check=True` raises exception on failure
- [ ] `cwd` and `env` parameters work correctly
- [ ] Tests pass
- [ ] Example script demonstrates typical usage patterns
