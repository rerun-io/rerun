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
        assert plan.nodes[0].kwargs.get("start") == 0
        assert "__taskclass_id__" in plan.nodes[0].kwargs  # Internal tracking

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


class TestTaskClassSerialization:
    """Test TaskClass state serialization/deserialization."""

    def test_write_and_read_taskclass_state(self, tmp_path: object) -> None:
        """TaskClass state can be serialized and deserialized."""
        from pathlib import Path
        import tempfile

        from recompose.workspace import read_taskclass_state, write_taskclass_state

        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)

            # Create a Counter instance directly (not in flow)
            counter = Counter(start=42)
            counter.count = 100  # Modify state

            # Serialize
            write_taskclass_state(workspace, "test_counter", counter)

            # Deserialize
            restored = read_taskclass_state(workspace, "test_counter")
            assert restored is not None
            assert isinstance(restored, Counter)
            assert restored.count == 100

    def test_taskclass_serialization_round_trip(self) -> None:
        """Complex TaskClass state survives round-trip."""
        from pathlib import Path
        import tempfile

        from recompose.workspace import read_taskclass_state, write_taskclass_state

        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)

            # Create and modify counter
            counter = Counter(start=0)
            counter.count = 12345

            # Round-trip through serialization
            write_taskclass_state(workspace, "counter_1", counter)
            restored = read_taskclass_state(workspace, "counter_1")

            assert restored is not None
            assert restored.count == 12345

            # Modify and round-trip again
            restored.count = 99999
            write_taskclass_state(workspace, "counter_1", restored)
            final = read_taskclass_state(workspace, "counter_1")

            assert final is not None
            assert final.count == 99999


class TestTaskClassRunStep:
    """Test run_step handling of TaskClass steps (unit tests, no subprocess)."""

    def test_init_step_creates_instance_and_serializes(self) -> None:
        """Running an __init__ step creates instance and serializes state."""
        from pathlib import Path
        import tempfile

        from recompose import flow
        from recompose.local_executor import run_step, setup_workspace
        from recompose.workspace import read_taskclass_state

        @flow
        def init_flow() -> None:
            counter = Counter(start=42)

        plan = init_flow.plan
        flow_info = init_flow._flow_info

        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)
            setup_workspace(flow_info, workspace=workspace)

            # Run the __init__ step directly (not via subprocess)
            result = run_step(flow_info, "step_1_counter.__init__", workspace)

            assert result.ok, f"Step failed: {result.error}"

            # Verify TaskClass state was serialized
            taskclass_id = plan.nodes[0].kwargs.get("__taskclass_id__")
            assert taskclass_id is not None

            restored = read_taskclass_state(workspace, taskclass_id)
            assert restored is not None
            assert isinstance(restored, Counter)
            assert restored.count == 42

    def test_method_step_deserializes_and_updates(self) -> None:
        """Running a method step deserializes instance, runs method, re-serializes."""
        from pathlib import Path
        import tempfile

        from recompose import flow
        from recompose.local_executor import run_step, setup_workspace
        from recompose.workspace import read_taskclass_state

        @flow
        def method_flow() -> None:
            counter = Counter(start=10)
            counter.increment(amount=5)

        plan = method_flow.plan
        flow_info = method_flow._flow_info

        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)
            setup_workspace(flow_info, workspace=workspace)

            taskclass_id = plan.nodes[0].kwargs.get("__taskclass_id__")
            assert taskclass_id is not None

            # Run __init__ step
            result1 = run_step(flow_info, "step_1_counter.__init__", workspace)
            assert result1.ok, f"Init step failed: {result1.error}"

            # Verify initial state
            counter = read_taskclass_state(workspace, taskclass_id)
            assert counter is not None
            assert counter.count == 10

            # Run increment step
            result2 = run_step(flow_info, "step_2_counter.increment", workspace)
            assert result2.ok, f"Increment step failed: {result2.error}"

            # Verify updated state
            counter = read_taskclass_state(workspace, taskclass_id)
            assert counter is not None
            assert counter.count == 15  # 10 + 5

    def test_chained_method_steps(self) -> None:
        """Running multiple method steps maintains correct state."""
        from pathlib import Path
        import tempfile

        from recompose import flow
        from recompose.local_executor import run_step, setup_workspace
        from recompose.workspace import read_taskclass_state

        @flow
        def chain_flow() -> None:
            counter = Counter(start=1)
            counter.increment(amount=2)
            counter.double()
            counter.increment(amount=3)

        plan = chain_flow.plan
        flow_info = chain_flow._flow_info

        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)
            setup_workspace(flow_info, workspace=workspace)

            taskclass_id = plan.nodes[0].kwargs.get("__taskclass_id__")
            assert taskclass_id is not None

            # Run all steps
            for i, node in enumerate(plan.nodes, 1):
                step_name = node.step_name or f"step_{i}_{node.name}"
                result = run_step(flow_info, step_name, workspace)
                assert result.ok, f"Step {step_name} failed: {result.error}"

            # Verify final state: 1 + 2 = 3, * 2 = 6, + 3 = 9
            counter = read_taskclass_state(workspace, taskclass_id)
            assert counter is not None
            assert counter.count == 9
