# P11: Explicit Command Registration

**Status: DONE**

**Goal:** Move from auto-registration to explicit, organized command registration with command groups and centralized configuration.

## Summary of Implementation

All phases completed successfully:
- **P11a**: Created `Config` dataclass and restructured `main()`
- **P11b**: Created `CommandGroup`, `builtin_commands()`, and grouped CLI help
- **P11c**: Migration complete, old auto-registration removed

Key changes:
1. **No global auto-registration** - `@task`, `@flow`, `@automation` decorators do NOT auto-register
2. **Explicit command list** - `main(commands=[...])` builds registry from explicit list
3. **CommandGroup** - Organizes commands with visual grouping in help output
4. **Config** - Centralized config for `python_cmd`, `working_directory`
5. **builtin_commands()** - Opt-in for inspect/generate-gha builtins
6. **Context-based registry** - `get_task_registry()`, `get_flow_registry()` read from context
7. **_recompose_tasks** - Dict on @taskclass classes for explicit registration

Files changed:
- `src/recompose/command_group.py` - NEW: Config and CommandGroup dataclasses
- `src/recompose/cli.py` - Updated main(), added GroupedClickGroup, _build_grouped_cli()
- `src/recompose/context.py` - Added RecomposeContext, registry getters
- `src/recompose/task.py` - Removed global registry, added _recompose_tasks to @taskclass
- `src/recompose/flow.py` - Removed global registry
- `src/recompose/automation.py` - Removed global registry
- `src/recompose/builtin_tasks.py` - Updated imports
- `examples/app.py` - Updated to new API
- All test files - Updated to use `._task_info`, `._flow_info` directly

All 180 tests pass.

## Target API

```python
import recompose
from . import flows, python_tasks, automations

if __name__ == "__main__":
    config = recompose.Config(
        python_cmd="uv run python",
        working_directory="recompose",
    )

    commands = [
        recompose.CommandGroup("Python", [
            python_tasks.lint,
            python_tasks.format,
            python_tasks.test,
        ]),
        recompose.CommandGroup("Rust", [...]),
        recompose.builtin_commands(),
        recompose.CommandGroup("Helpers", [
            flows.pre_push_checks,
        ], hidden=True),
    ]

    automations = [
        automations.on_pr,
        automations.nightly,
    ]

    recompose.main(
        config=config,
        commands=commands,
        automations=automations,
    )
```

## Key Design Decisions

1. **Flat namespace** - All commands accessible as `./run <command>`, not `./run python.lint`
   - Groups only affect help output organization, not command names
2. **Visual grouping in help** - Commands organized under group headings
3. **Hidden groups** - Some commands available but not shown in default help
4. **Explicit control** - Only listed tasks appear as CLI commands
5. **Internal tasks** - Tasks can still be used by flows without CLI exposure

## Implementation Plan

### P11a - Config class and restructured main()

**Create `Config` class:**
```python
# src/recompose/config.py
from dataclasses import dataclass

@dataclass
class Config:
    """Configuration for recompose CLI."""
    python_cmd: str = "python"
    working_directory: str | None = None
    # Room for future config options
```

**Update `main()` signature:**
```python
# src/recompose/cli.py
def main(
    *,
    config: Config | None = None,
    commands: list[CommandGroup | TaskWrapper] | None = None,
    automations: list[AutomationWrapper] | None = None,
) -> None:
    """Build and run the recompose CLI."""
    # If commands is None, use old behavior (all registered tasks)
    # for backwards compatibility during migration
```

**Migration notes:**
- Keep old `main(python_cmd=..., working_directory=...)` working temporarily
- Deprecation warning if old style used
- Remove old style after migration complete

**Completion criteria:**
- Config class exists
- main() accepts new parameters
- Old API still works with deprecation warning
- Tests pass

### P11b - CommandGroup and explicit registration

**Create `CommandGroup` class:**
```python
# src/recompose/command_group.py
class CommandGroup:
    """Groups commands under a heading in help output."""

    def __init__(
        self,
        name: str,
        commands: list[TaskWrapper | FlowWrapper],
        *,
        hidden: bool = False,
    ):
        self.name = name
        self.commands = commands
        self.hidden = hidden

    def get_commands(self) -> list[TaskWrapper | FlowWrapper]:
        """Return all commands in this group."""
        return self.commands
```

**Create `builtin_commands()` function:**
```python
# src/recompose/builtin_tasks.py
def builtin_commands() -> CommandGroup:
    """Returns a CommandGroup with all built-in commands."""
    return CommandGroup("Built-in", [
        generate_gha,
        inspect,
        # ... other builtins
    ])
```

**Update CLI generation:**
```python
# src/recompose/cli.py
def _build_cli(
    commands: list[CommandGroup | TaskWrapper],
    config: Config,
) -> click.Group:
    """Build Click CLI from command groups."""

    # Flatten all commands while tracking groups
    command_to_group: dict[str, str] = {}
    all_commands: list[TaskWrapper] = []

    for item in commands:
        if isinstance(item, CommandGroup):
            for cmd in item.commands:
                all_commands.append(cmd)
                command_to_group[cmd.name] = item.name
        else:
            all_commands.append(item)
            command_to_group[item.name] = "Other"

    # Build flat CLI (same as before)
    cli = click.Group(help="Recompose task runner")
    for cmd in all_commands:
        cli.add_command(_make_click_command(cmd, config))

    # Store group metadata for help formatting
    cli.command_groups = command_to_group

    return cli
```

**Update help formatting:**
```python
# Override Click's format_commands() to show groups
class GroupedGroup(click.Group):
    def format_commands(self, ctx, formatter):
        """Format commands grouped by category."""
        # Group commands by their group name
        groups = {}
        for name, cmd in self.commands.items():
            group_name = self.command_groups.get(name, "Other")
            if group_name not in groups:
                groups[group_name] = []
            groups[group_name].append((name, cmd))

        # Format each group
        for group_name, commands in groups.items():
            with formatter.section(group_name):
                formatter.write_dl([
                    (name, cmd.get_short_help_str(limit=45))
                    for name, cmd in commands
                ])
```

**Completion criteria:**
- CommandGroup class works
- builtin_commands() returns built-in tasks
- CLI generation uses groups for help
- Help output shows grouped commands
- Commands still in flat namespace
- Hidden groups don't show in default help
- Tests pass

### P11c - Migration and validation

**Update examples/app.py:**
```python
#!/usr/bin/env python3
import recompose
from examples.tasks import lint, test, build
from examples.flows import ci

if __name__ == "__main__":
    config = recompose.Config(
        python_cmd="uv run python",
        working_directory="recompose",
    )

    commands = [
        recompose.CommandGroup("Quality", [
            lint.lint,
            lint.format_check,
            lint.format,
        ]),
        recompose.CommandGroup("Testing", [
            test.test,
        ]),
        recompose.CommandGroup("Build", [
            build.build_wheel,
            build.test_installed,
        ]),
        recompose.CommandGroup("Flows", [
            ci.ci,
        ]),
        recompose.builtin_commands(),
    ]

    recompose.main(config=config, commands=commands)
```

**Verify key behaviors:**
1. Only listed commands appear in CLI
2. Tasks used by flows still work (internal registry separate from CLI)
3. Hidden groups work correctly
4. Help output is well-organized
5. Backwards compat for old-style main() during transition

**Update tests:**
- Test CommandGroup creation and get_commands()
- Test builtin_commands() returns expected builtins
- Test CLI generation with groups
- Test help formatting shows groups correctly
- Test hidden groups don't appear in help
- Test flat namespace (no nested commands)
- Test flows can use non-CLI tasks

**Remove old auto-registration:**
- Remove auto-registration from @task decorator (for CLI)
- Keep internal task registry (for flows to reference tasks)
- Remove backwards-compat code after migration

**Completion criteria:**
- examples/app.py uses new API
- All tests updated and passing
- Help output looks good with groups
- Hidden groups work
- Non-CLI tasks usable by flows
- Documentation updated

## Design Notes

### Why flat namespace?

Groups are purely organizational in the help output. This keeps commands simple:
```bash
./run lint          # Not ./run python lint or ./run python.lint
./run test
./run build_wheel
```

If we need namespacing later, we can add it, but start simple.

### Hidden vs internal

- **Hidden commands**: In CLI but not shown in default help (use `--show-hidden`)
- **Internal tasks**: Not in CLI at all, only usable programmatically/by flows

### Future enhancements

Could add later if needed:
- Command aliases (e.g., `fmt` -> `format`)
- Per-command visibility rules (not just group-level)
- Nested command groups (subgroups)
- Dynamic command generation
- Command deprecation warnings

## Open Questions

1. Should `builtin_commands()` be a CommandGroup or just a list?
   - Leaning toward CommandGroup for consistency
2. How to handle `inspect` command? It's built-in but special
   - Keep it as built-in, or make it always available?
3. Should hidden groups accept `--show-hidden` flag, or different flag?
   - `--all` might be clearer?
4. Error handling for duplicate command names across groups?
   - Should fail fast with clear error message

## Dependencies

- None - this is a refactor of existing functionality

## Testing Strategy

1. Unit tests for Config, CommandGroup classes
2. Unit tests for builtin_commands()
3. Integration tests for CLI generation with groups
4. Integration tests for help formatting
5. Test that flows can use non-CLI tasks
6. Test backwards compatibility during migration
