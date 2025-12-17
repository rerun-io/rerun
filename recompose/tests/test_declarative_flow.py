"""Tests for declarative flow execution."""

from recompose import FlowPlan, Ok, Result, flow, task

from . import flow_test_app

# =============================================================================
# Execution tests (use module-level flows for subprocess compatibility)
# =============================================================================


def test_declarative_flow_basic():
    """Test basic declarative flow execution."""
    result = flow_test_app.simple_flow()
    assert result.ok
    assert result.value() is None  # Flows return None


def test_declarative_flow_with_dependencies():
    """Test declarative flow with task dependencies using .value() pattern."""
    result = flow_test_app.dependent_flow()
    assert result.ok


def test_declarative_flow_with_arguments():
    """Test declarative flow with external arguments."""
    result = flow_test_app.arg_flow(initial=21)
    assert result.ok


def test_declarative_flow_fail_fast():
    """Test that declarative flows fail fast when a task fails."""
    result = flow_test_app.fail_fast_flow()
    assert result.failed
    assert "failed!" in (result.error or "")


# =============================================================================
# Plan-only tests (no subprocess execution needed)
# =============================================================================


def test_flow_plan_method():
    """Test that flows have a .plan() method for dry-run."""

    @task
    def plan_task_a() -> Result[str]:
        return Ok("a")

    @task
    def plan_task_b(*, from_a: str) -> Result[str]:
        return Ok(f"b from {from_a}")

    @flow
    def plannable_flow() -> None:
        a = plan_task_a()
        plan_task_b(from_a=a.value())

    # Get the plan without executing
    plan = plannable_flow.plan()

    assert isinstance(plan, FlowPlan)
    assert len(plan.nodes) == 2
    assert plan.terminal is not None
    assert plan.terminal.name == "plan_task_b"


def test_flow_plan_shows_dependencies():
    """Test that flow plan correctly shows dependencies."""

    @task
    def dep_root() -> Result[int]:
        return Ok(1)

    @task
    def dep_child(*, val: int) -> Result[int]:
        return Ok(val + 1)

    @flow
    def dep_flow() -> None:
        root = dep_root()
        dep_child(val=root.value())

    plan = dep_flow.plan()

    # Find the child node
    child_node = next(n for n in plan.nodes if n.name == "dep_child")
    assert len(child_node.dependencies) == 1
    assert child_node.dependencies[0].name == "dep_root"


def test_flow_plan_execution_order():
    """Test that flow plan provides valid execution order."""

    @task
    def order_a() -> Result[int]:
        return Ok(1)

    @task
    def order_b(*, a: int) -> Result[int]:
        return Ok(a + 1)

    @task
    def order_c(*, b: int) -> Result[int]:
        return Ok(b + 1)

    @flow
    def ordered_plan_flow() -> None:
        a = order_a()
        b = order_b(a=a.value())
        order_c(b=b.value())

    plan = ordered_plan_flow.plan()

    # Verify order: a before b before c
    # Nodes are in valid execution order by construction
    names = [n.name for n in plan.nodes]
    assert names.index("order_a") < names.index("order_b")
    assert names.index("order_b") < names.index("order_c")


def test_task_node_repr():
    """Test TaskNode string representation."""

    @task
    def repr_task() -> Result[str]:
        return Ok("done")

    @flow
    def repr_flow() -> None:
        repr_task()

    plan = repr_flow.plan()
    node = plan.nodes[0]

    node_repr = repr(node)
    assert "TaskNode" in node_repr
    assert "repr_task" in node_repr
