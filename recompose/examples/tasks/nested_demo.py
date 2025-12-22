"""
Demo of deeply nested tasks to test output formatting.

This creates a 3-level nesting structure to verify the tree output looks correct.
"""

import recompose


@recompose.task
def level_3a() -> recompose.Result[None]:
    """Deepest level task A."""
    recompose.out("Doing level 3a work...")
    recompose.out("Level 3a complete!")
    return recompose.Ok(None)


@recompose.task
def level_3b() -> recompose.Result[None]:
    """Deepest level task B."""
    recompose.out("Doing level 3b work...")
    recompose.out("Level 3b complete!")
    return recompose.Ok(None)


@recompose.task
def level_2a() -> recompose.Result[None]:
    """Middle level task A - calls level 3 tasks."""
    recompose.out("Starting level 2a...")

    result = level_3a()
    if result.failed:
        return result

    result = level_3b()
    if result.failed:
        return result

    recompose.out("Level 2a complete!")
    return recompose.Ok(None)


@recompose.task
def level_2b() -> recompose.Result[None]:
    """Middle level task B - no subtasks."""
    recompose.out("Doing level 2b work...")
    recompose.out("Level 2b complete!")
    return recompose.Ok(None)


@recompose.task
def level_1() -> recompose.Result[None]:
    """Top level task - calls level 2 tasks."""
    recompose.out("Starting nested demo...")

    result = level_2a()
    if result.failed:
        return result

    result = level_2b()
    if result.failed:
        return result

    recompose.out("All nested tasks complete!")
    return recompose.Ok(None)
