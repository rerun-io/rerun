"""Tests for declarative flow execution (P05b)."""

import pytest

import recompose
from recompose import Err, FlowPlan, Ok, Result, TaskNode, flow, task


def test_task_has_flow_method():
    """Test that @task decorated functions have a .flow() method."""

    @task
    def my_task() -> Result[str]:
        return Ok("done")

    assert hasattr(my_task, "flow")
    assert callable(my_task.flow)


def test_flow_method_raises_outside_flow():
    """Test that .flow() raises RuntimeError when called outside a flow."""

    @task
    def standalone_task() -> Result[str]:
        return Ok("done")

    with pytest.raises(RuntimeError, match="can only be called inside"):
        standalone_task.flow()


def test_declarative_flow_basic():
    """Test basic declarative flow execution."""

    @task
    def step_a() -> Result[str]:
        return Ok("a_result")

    @task
    def step_b() -> Result[str]:
        return Ok("b_result")

    @flow
    def simple_declarative():
        a = step_a.flow()
        b = step_b.flow()
        return b  # Return terminal node

    result = simple_declarative()
    assert result.ok
    assert result.value == "b_result"


def test_declarative_flow_with_dependencies():
    """Test declarative flow with task dependencies."""

    @task
    def produce(*, value: int) -> Result[int]:
        return Ok(value * 2)

    @task
    def consume(*, input_val: int) -> Result[str]:
        return Ok(f"got {input_val}")

    @flow
    def dependent_flow():
        produced = produce.flow(value=5)
        consumed = consume.flow(input_val=produced)  # Depends on produced
        return consumed

    result = dependent_flow()
    assert result.ok
    assert result.value == "got 10"


def test_declarative_flow_execution_order():
    """Test that declarative flows execute in topological order."""
    execution_order = []

    @task
    def task_first() -> Result[int]:
        execution_order.append("first")
        return Ok(1)

    @task
    def task_second(*, from_first: int) -> Result[int]:
        execution_order.append("second")
        return Ok(from_first + 1)

    @task
    def task_third(*, from_second: int) -> Result[int]:
        execution_order.append("third")
        return Ok(from_second + 1)

    @flow
    def ordered_flow():
        first = task_first.flow()
        second = task_second.flow(from_first=first)
        third = task_third.flow(from_second=second)
        return third

    execution_order.clear()
    result = ordered_flow()

    assert result.ok
    assert result.value == 3
    assert execution_order == ["first", "second", "third"]


def test_declarative_flow_parallel_structure():
    """Test declarative flow with parallel task structure."""
    execution_order = []

    @task
    def source_task() -> Result[int]:
        execution_order.append("source")
        return Ok(10)

    @task
    def branch_a(*, val: int) -> Result[int]:
        execution_order.append("branch_a")
        return Ok(val + 1)

    @task
    def branch_b(*, val: int) -> Result[int]:
        execution_order.append("branch_b")
        return Ok(val + 2)

    @task
    def merge_task(*, a: int, b: int) -> Result[int]:
        execution_order.append("merge")
        return Ok(a + b)

    @flow
    def diamond_flow():
        src = source_task.flow()
        a = branch_a.flow(val=src)
        b = branch_b.flow(val=src)
        merged = merge_task.flow(a=a, b=b)
        return merged

    execution_order.clear()
    result = diamond_flow()

    assert result.ok
    assert result.value == 23  # (10+1) + (10+2) = 23
    assert execution_order[0] == "source"
    assert "merge" in execution_order[-1]


def test_declarative_flow_fail_fast():
    """Test that declarative flows fail fast when a task fails."""
    execution_order = []

    @task
    def ok_task() -> Result[str]:
        execution_order.append("ok")
        return Ok("fine")

    @task
    def failing_task() -> Result[str]:
        execution_order.append("fail")
        return Err("failed!")

    @task
    def never_run() -> Result[str]:
        execution_order.append("never")
        return Ok("should not see this")

    @flow
    def fail_fast_flow():
        ok = ok_task.flow()
        bad = failing_task.flow()
        after = never_run.flow()
        return after

    execution_order.clear()
    result = fail_fast_flow()

    assert result.failed
    assert result.error == "failed!"
    assert "never" not in execution_order


def test_flow_plan_method():
    """Test that flows have a .plan() method for dry-run."""

    @task
    def plan_task_a() -> Result[str]:
        return Ok("a")

    @task
    def plan_task_b(*, from_a: str) -> Result[str]:
        return Ok(f"b from {from_a}")

    @flow
    def plannable_flow():
        a = plan_task_a.flow()
        b = plan_task_b.flow(from_a=a)
        return b

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
    def dep_flow():
        root = dep_root.flow()
        child = dep_child.flow(val=root)
        return child

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
    def ordered_plan_flow():
        a = order_a.flow()
        b = order_b.flow(a=a)
        c = order_c.flow(b=b)
        return c

    plan = ordered_plan_flow.plan()
    order = plan.get_execution_order()

    # Verify order: a before b before c
    names = [n.name for n in order]
    assert names.index("order_a") < names.index("order_b")
    assert names.index("order_b") < names.index("order_c")


def test_flow_plan_parallelizable_groups():
    """Test that flow plan identifies parallelizable groups."""

    @task
    def parallel_root() -> Result[int]:
        return Ok(1)

    @task
    def parallel_a(*, val: int) -> Result[int]:
        return Ok(val + 1)

    @task
    def parallel_b(*, val: int) -> Result[int]:
        return Ok(val + 2)

    @flow
    def parallel_flow():
        root = parallel_root.flow()
        a = parallel_a.flow(val=root)
        b = parallel_b.flow(val=root)
        return a  # Doesn't matter which we return

    plan = parallel_flow.plan()
    groups = plan.get_parallelizable_groups()

    # Level 0: root
    # Level 1: a, b (can run in parallel)
    assert len(groups) == 2
    assert len(groups[0]) == 1  # Just root
    assert len(groups[1]) == 2  # a and b


def test_flow_plan_visualize():
    """Test that flow plan can be visualized."""

    @task
    def viz_task() -> Result[str]:
        return Ok("done")

    @flow
    def viz_flow():
        return viz_task.flow()

    plan = viz_flow.plan()
    viz = plan.visualize()

    assert isinstance(viz, str)
    assert "viz_task" in viz


def test_declarative_flow_with_arguments():
    """Test declarative flow with external arguments."""

    @task
    def double(*, value: int) -> Result[int]:
        return Ok(value * 2)

    @flow
    def arg_flow(*, initial: int):
        doubled = double.flow(value=initial)
        return doubled

    result = arg_flow(initial=21)
    assert result.ok
    assert result.value == 42


def test_declarative_flow_tracks_executions():
    """Test that declarative flow tracks task executions."""

    @task
    def tracked_a() -> Result[str]:
        return Ok("a")

    @task
    def tracked_b() -> Result[str]:
        return Ok("b")

    @flow
    def tracking_flow():
        a = tracked_a.flow()
        b = tracked_b.flow()
        return b

    result = tracking_flow()
    assert result.ok

    # Check flow context was attached
    flow_ctx = getattr(result, "_flow_context", None)
    assert flow_ctx is not None
    assert len(flow_ctx.executions) == 2


def test_declarative_flow_attaches_plan():
    """Test that executed declarative flow attaches the plan."""

    @task
    def attached_task() -> Result[str]:
        return Ok("done")

    @flow
    def attached_flow():
        return attached_task.flow()

    result = attached_flow()
    assert result.ok

    # Check plan was attached
    plan = getattr(result, "_flow_plan", None)
    assert plan is not None
    assert isinstance(plan, FlowPlan)


def test_direct_task_call_in_flow_raises():
    """Test that calling a task directly inside a flow raises an error."""

    @task
    def direct_call_task() -> Result[str]:
        return Ok("done")

    @flow
    def bad_flow():
        direct_call_task()  # Direct call, should raise
        return direct_call_task.flow()

    with pytest.raises(recompose.DirectTaskCallInFlowError):
        bad_flow()


def test_task_node_repr():
    """Test TaskNode string representation."""

    @task
    def repr_task() -> Result[str]:
        return Ok("done")

    @flow
    def repr_flow():
        return repr_task.flow()

    plan = repr_flow.plan()
    node = plan.nodes[0]

    node_repr = repr(node)
    assert "TaskNode" in node_repr
    assert "repr_task" in node_repr
