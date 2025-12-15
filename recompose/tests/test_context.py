"""Tests for the context and output helpers."""

from recompose import Ok, Result, dbg, get_context, is_debug, out, set_debug, task


def test_out_works_outside_task(capsys):
    out("Hello from outside")
    captured = capsys.readouterr()
    assert "Hello from outside" in captured.out


def test_dbg_silent_by_default(capsys):
    set_debug(False)
    dbg("Debug message")
    captured = capsys.readouterr()
    assert "Debug message" not in captured.out


def test_dbg_prints_when_enabled(capsys):
    set_debug(True)
    dbg("Debug message")
    captured = capsys.readouterr()
    assert "Debug message" in captured.out
    set_debug(False)  # Reset


def test_context_exists_inside_task():
    ctx_inside = None

    @task
    def context_task() -> Result[str]:
        nonlocal ctx_inside
        ctx_inside = get_context()
        return Ok("done")

    context_task()
    assert ctx_inside is not None
    assert ctx_inside.task_name == "context_task"


def test_context_none_outside_task():
    assert get_context() is None


def test_output_captured_in_context():
    @task
    def capturing_task() -> Result[str]:
        out("Line 1")
        out("Line 2")
        dbg("Debug line")
        ctx = get_context()
        assert ctx is not None
        return Ok(str(len(ctx.output)))

    result = capturing_task()
    assert result.ok
    # 2 out lines + 1 dbg line = 3 total
    assert result.value == "3"


def test_is_debug():
    set_debug(False)
    assert is_debug() is False
    set_debug(True)
    assert is_debug() is True
    set_debug(False)  # Reset
