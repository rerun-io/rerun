# GOALS

-   Implement a new library/helper: `recompose`
-   Don't break any existing code. Do all of this as a new parallel system next to the current status quo.
-   Demonstrate some mvp tasks that show `recompose` can be used to replace ALL OF:
    -   `pixi` tasks
    -   `scripts/`
    -   Job-steps in the CI jobs

# Vision

Recompose is a light-weight typed, pythonic, task execution framework.
It's inspired by tools such as dagger CI and pydoit, but special-tailored to meet
a particular style of developer workflow.

The end-vision is that recompose is used to define all "tasks" within a project, whether
they are run in CI, or locally as part of a development workflow.

Architecturally, the goal is that each task should just be a python function with a
special `@recompose.task` decorator. The intention is that these functions can still be
imported and used programmatically in a way that is not surprising. However, the decorator
should unlock a significant amount of additional convenience.

The first primary function of recompose is to allow a user to build a helper application that
exposes all of the registered tasks as sub-domains. My thought is that the user should be
responsible for creating the application since this makes packaging and imports explicit.

Exa:

rerun_recompose.py:

```
#!/usr/bin/env python3

import recompose
import my_task_package
import my_other_package

@recompose.task
def inline_task(*, arg1: str, arg2: int = 42) -> recompose.Result[float]:
    recompose.dbg(f"This is a debug line!")
    recompose.out(f"Hello from task {arg1}, {arg2}")

    return recompose.Ok(0.57)

recompose.main()
```

Then I should be able to do something like

```
> ./rerun_recompose.py inline_task --arg1="FOO"

▶️ rerun_recompose:inline_task:

Hello from task FOO, 42

🟢 rerun_compose:inline_task SUCCEEDED in 0.05 sec
-> 0.57

All tasks completed. Full logs in: ~/.recompose/logs/inline_task_2025_12_07_13_34_09.txt
```

## Additional features

### Helpful context object

There are useful helpers like `recompose.out` which store state in the environment only if the
task is being executed INSIDE the recompose engine. Otherwise these fallback on a non-recompose
implementation like print. This means you can mostly use recompose tasks as functions if you want
to, but when you use them as a task or flow then you get improved behavior.

### Smart Result type

`recompose.Result` encompasses all of the outputs of a task, including status code, captured
outputs, and structured/typed results. For complex return types users should subclass recompose.Result
via pydantic.

Results use an immutable factory pattern:
- `recompose.Ok(value)` - creates a successful result with the given value
- `recompose.Err(message)` - creates a failure result with an error message
- `recompose.Result[T]` - the base type for type hints

The `@task` decorator wrapper automatically catches any uncaught exceptions and converts them
to `Err` results with the exception message and traceback. This means tasks don't need explicit
try/except blocks unless they want custom error handling.

### Ergonomic primitives

Where appropriate, recompose should offer convenient helpers types that provide additional utility.
For example a `recompose.Artifact(path: Path)` lets us track when a task produces an artifact,
in cases where another task might depend on it. This would allow other tasks, or mechanisms like
auto-github-runners to track additional information and facilitate primitives like upload or download.

### Easy subprocess runner

A very common job for task execution frameworks is calling subprocesses like `uv`, `cargo`, etc.
`recompose` should come with good built-in tools and primitives making it easy to write these kinds
of tasks.

### Member-tasks

As an object-oriented language, it's convenient if we can create objects, which have tasks as members
which when run have access to the context of the object. The signatures for such tasks are more complicated.
Each member-task should also include the CLI arguments for the base `__init__` task.

```
PROJ_BASE = recompose.root() / 'pyproject.toml'

class Venv:
    location: Path

    @recompose.task
    def __init__(location: Path, proj = PROJ_BASE, clean: bool = False):
        # Bootstrap the uv environment

    @recompose.task
    def sync(self, group = None, package: str = None) -> recompose.Result[Venv]:
        # Run uv sync with the right args

        return recompose.Ok(self)

    @recompose.task
    def install(self, wheel: recompose.Artifact) -> recompose.Result[Venv]
        # Install the provided wheel

        return recompose.Ok(self)

    @recompose.task
    def run(self, wheel: recompose.Artifact) -> recompose.Result[Venv]
        # Install the provided wheel

        return recompose.Ok(self)
```

### Task-dependencies via "Flows"

While tasks make up composable units that are sometimes useful on their, own, often times we want
to compose flows of several tasks. Flows LOOK like we're just writing a program but they are actually
executed at construction time. This is why they can only take `recompose.Input`-based arguments, and
should internally only execute tasks. When dispatched within the load-time flow, the task decorator
should do the right thing and only operate based on Input/Result signature place-holders to evaluate
the graph. Flows should run each task via `subprocess`. We should be clever in how we pass input/output
between tasks here. If this is all defined cleanly, we should be able to map this to a sequential
execution of tasks via ANY runner. This means a flow could be used to, for example, render a bash
script (or github actions workflow). Each step in the script invokes the task (using the top-level
recompose entrypoint). To make this convenient we probably need to have alternative dispatch mechanisms.
For example, the result object could be written into a temp object in a similar fashion to how GITHUB_OUTPUT
works.

```

@recompose.flow
def test_wheel(location: recompose.Input[Path] = recompose.temp_dir())
    wheel = py_build_wheel()

    venv = create_test_env(location=location)

    venv.install(wheel)

    py_test(venv=venv)
```

### Flow-generate github actions

The last killer feature we really want is the ability to use recompose flows to generate
github action using the flow spec. Each task in the flow maps directly to a step within the github
workflow. We probably need 1 or 2 place-holder tasks that are only relevant to github to correspond
to actions like setup-pixi or setup-rust. These could be skipped when running flows locally but
would allow the flow to properly render the workflows.

It would be super helpful if recompose then included a CLI tool (built on top of github CLI) to
find and inspect the flows that it knows about and map their status all the way back to a the
representation used to show the results of a local run of a flow.

Lastly we probably need a way to handle "secrets" and "env-vars" for local runs. It seems totally
reasonable to have a local config file that handles this. This means a flow / task could
specify it depends on those config values existing and fail early if they aren't set up.
