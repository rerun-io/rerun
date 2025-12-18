"""Tests for TaskClass usage in flows."""

from recompose import Ok, Result, flow, task, taskclass


@taskclass
class Counter:
    """A simple counter for testing TaskClass in flows."""

    def __init__(self, *, start: int = 0):
        self.count = start

    @task
    def increment(self, *, amount: int = 1) -> Result[int]:
        """Increment the counter and return the new value."""
        self.count += amount
        return Ok(self.count)

    @task
    def double(self) -> Result[int]:
        """Double the counter value."""
        self.count *= 2
        return Ok(self.count)

    def get_count(self) -> int:
        """Get the current count (regular method)."""
        return self.count


class TestTaskClassDirectUsage:
    """Test TaskClass usage outside of flows."""

    def test_direct_instantiation(self) -> None:
        """TaskClass can be instantiated directly."""
        counter = Counter(start=5)
        assert counter.count == 5

    def test_direct_task_method_call(self) -> None:
        """Task methods execute immediately when called directly."""
        counter = Counter(start=10)
        result = counter.increment(amount=5)
        assert result.ok
        assert result.value() == 15
        assert counter.count == 15

    def test_direct_regular_method_call(self) -> None:
        """Regular methods work normally."""
        counter = Counter(start=7)
        assert counter.get_count() == 7


class TestTaskClassInFlow:
    """Test TaskClass usage inside flows."""

    def test_taskclass_in_flow_creates_nodes(self) -> None:
        """Instantiating a TaskClass in a flow creates TaskNodes."""

        @flow
        def counter_flow() -> None:
            counter = Counter(start=0)
            counter.increment(amount=5)

        plan = counter_flow.plan
        assert len(plan.nodes) == 2

        # First node is __init__
        assert plan.nodes[0].task_info.name == "counter.__init__"
        assert plan.nodes[0].kwargs == {"start": 0}

        # Second node is increment
        assert plan.nodes[1].task_info.name == "counter.increment"
        assert plan.nodes[1].kwargs.get("amount") == 5

    def test_method_depends_on_init(self) -> None:
        """Method calls depend on __init__."""

        @flow
        def counter_flow() -> None:
            counter = Counter(start=0)
            counter.increment(amount=5)

        plan = counter_flow.plan
        init_node = plan.nodes[0]
        increment_node = plan.nodes[1]

        # increment should depend on init
        assert init_node in increment_node.dependencies

    def test_chained_methods_have_correct_dependencies(self) -> None:
        """Chained method calls depend on previous method."""

        @flow
        def counter_flow() -> None:
            counter = Counter(start=1)
            counter.increment(amount=2)
            counter.double()
            counter.increment(amount=3)

        plan = counter_flow.plan
        assert len(plan.nodes) == 4

        init_node = plan.nodes[0]
        inc1_node = plan.nodes[1]
        double_node = plan.nodes[2]
        inc2_node = plan.nodes[3]

        # Check dependency chain
        assert init_node in inc1_node.dependencies
        assert inc1_node in double_node.dependencies
        assert double_node in inc2_node.dependencies

    def test_taskclass_node_proxy_blocks_regular_methods(self) -> None:
        """Regular methods cannot be called in flow context."""

        try:

            @flow
            def bad_flow() -> None:
                counter = Counter(start=0)
                counter.get_count()  # This should fail

            # Should raise during flow decoration (plan building)
            assert False, "Expected AttributeError"
        except AttributeError as e:
            assert "get_count" in str(e)
            assert "Only @task-decorated methods" in str(e)


class TestTaskClassPassedToTask:
    """Test passing TaskClass to other tasks."""

    def test_taskclass_passed_to_task(self) -> None:
        """TaskClass can be passed to other tasks."""

        @task
        def use_counter(*, counter: Counter) -> Result[int]:
            return Ok(counter.get_count())

        @flow
        def flow_with_taskclass() -> None:
            counter = Counter(start=42)
            use_counter(counter=counter)

        plan = flow_with_taskclass.plan
        assert len(plan.nodes) == 2

        init_node = plan.nodes[0]
        use_node = plan.nodes[1]

        # use_counter should depend on Counter.__init__
        assert init_node in use_node.dependencies

    def test_taskclass_passed_after_method_calls(self) -> None:
        """Passing TaskClass after method calls depends on last method."""

        @task
        def use_counter(*, counter: Counter) -> Result[int]:
            return Ok(counter.get_count())

        @flow
        def flow_with_methods() -> None:
            counter = Counter(start=0)
            counter.increment(amount=10)
            counter.double()
            use_counter(counter=counter)

        plan = flow_with_methods.plan
        assert len(plan.nodes) == 4

        double_node = plan.nodes[2]
        use_node = plan.nodes[3]

        # use_counter should depend on double (the last method call)
        assert double_node in use_node.dependencies


class TestTaskClassStepNames:
    """Test step name assignment for TaskClass nodes."""

    def test_step_names_assigned_correctly(self) -> None:
        """TaskClass nodes get proper step names."""

        @flow
        def counter_flow() -> None:
            counter = Counter(start=0)
            counter.increment(amount=5)

        plan = counter_flow.plan
        steps = plan.get_steps()

        assert len(steps) == 2
        assert "counter.__init__" in steps[0][0]
        assert "counter.increment" in steps[1][0]
