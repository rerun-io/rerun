"""Tests for automation decorator and workflow generation."""

import pytest
from ruamel.yaml import YAML

import recompose
from recompose.automation import (
    AutomationPlan,
    get_automation,
    get_automation_registry,
)
from recompose.gha import render_automation_workflow


# Test fixtures - flows for testing
@recompose.task
def build_task() -> recompose.Result[str]:
    """A simple build task."""
    return recompose.Ok("built")


@recompose.flow
def build_flow(*, repo: str = "main") -> None:
    """A flow to build."""
    build_task()


@recompose.flow
def run_tests_flow() -> None:
    """A flow to run tests."""
    build_task()


# Test automations
@recompose.automation
def simple_automation():
    """A simple automation with no config."""
    build_flow.dispatch()


@recompose.automation(
    gha_on={"schedule": [{"cron": "0 0 * * *"}]},
    gha_runs_on="ubuntu-latest",
)
def scheduled_automation():
    """An automation with schedule trigger."""
    build_flow.dispatch(repo="main")
    run_tests_flow.dispatch()


@recompose.automation(
    gha_on={"push": {"branches": ["main"]}},
    gha_env={"DEBUG": "true"},
    gha_timeout_minutes=30,
)
def push_automation():
    """An automation triggered on push."""
    build_flow.dispatch(repo="main")


class TestFlowDispatch:
    """Tests for FlowDispatch."""

    def test_dispatch_outside_automation_raises(self) -> None:
        """Test that .dispatch() outside automation raises."""
        with pytest.raises(RuntimeError, match="can only be called inside"):
            build_flow.dispatch()

    def test_dispatch_records_params(self) -> None:
        """Test that dispatch records parameters."""
        plan = scheduled_automation.plan()  # type: ignore[union-attr]

        assert len(plan.dispatches) == 2
        assert plan.dispatches[0].flow_name == "build_flow"
        assert plan.dispatches[0].params == {"repo": "main"}
        assert plan.dispatches[1].flow_name == "run_tests_flow"
        assert plan.dispatches[1].params == {}


class TestAutomationDecorator:
    """Tests for @automation decorator."""

    def test_automation_registers(self) -> None:
        """Test that automation registers in registry."""
        info = get_automation("simple_automation")
        assert info is not None
        assert info.name == "simple_automation"

    def test_automation_with_config(self) -> None:
        """Test automation with GHA config."""
        info = get_automation("scheduled_automation")
        assert info is not None
        assert info.gha_on == {"schedule": [{"cron": "0 0 * * *"}]}
        assert info.gha_runs_on == "ubuntu-latest"

    def test_automation_plan(self) -> None:
        """Test automation.plan() returns plan."""
        plan = simple_automation.plan()  # type: ignore[union-attr]
        assert isinstance(plan, AutomationPlan)
        assert len(plan.dispatches) == 1

    def test_automation_callable(self) -> None:
        """Test automation is callable (builds plan)."""
        # Calling the automation should not raise
        simple_automation()  # type: ignore[call-arg]


class TestRenderAutomationWorkflow:
    """Tests for automation YAML generation."""

    def test_simple_automation_yaml(self) -> None:
        """Test YAML generation for simple automation."""
        info = get_automation("simple_automation")
        assert info is not None

        spec = render_automation_workflow(info)

        assert spec.name == "simple_automation"
        # Default trigger is workflow_dispatch
        assert "workflow_dispatch" in spec.on

        job = spec.jobs["simple_automation"]
        assert job.runs_on == "ubuntu-latest"
        # Checkout + 1 dispatch
        assert len(job.steps) == 2

    def test_scheduled_automation_yaml(self) -> None:
        """Test YAML generation with schedule trigger."""
        info = get_automation("scheduled_automation")
        assert info is not None

        spec = render_automation_workflow(info)

        # Check schedule trigger
        assert "schedule" in spec.on
        assert spec.on["schedule"][0]["cron"] == "0 0 * * *"

        job = spec.jobs["scheduled_automation"]
        # Checkout + 2 dispatches
        assert len(job.steps) == 3

        # Check dispatch steps
        dispatch_steps = [s for s in job.steps if s.name.startswith("Dispatch")]
        assert len(dispatch_steps) == 2
        assert "build_flow" in dispatch_steps[0].name
        assert "run_tests_flow" in dispatch_steps[1].name

    def test_push_automation_yaml(self) -> None:
        """Test YAML generation with push trigger and env."""
        info = get_automation("push_automation")
        assert info is not None

        spec = render_automation_workflow(info)

        # Check push trigger
        assert "push" in spec.on
        assert spec.on["push"]["branches"] == ["main"]

        job = spec.jobs["push_automation"]
        assert job.env == {"DEBUG": "true"}
        assert job.timeout_minutes == 30

    def test_dispatch_step_has_gh_token(self) -> None:
        """Test that dispatch steps have GH_TOKEN env."""
        info = get_automation("simple_automation")
        assert info is not None

        spec = render_automation_workflow(info)

        job = spec.jobs["simple_automation"]
        dispatch_step = [s for s in job.steps if s.name.startswith("Dispatch")][0]
        assert dispatch_step.env is not None
        assert "GH_TOKEN" in dispatch_step.env

    def test_dispatch_with_params_uses_json(self) -> None:
        """Test that dispatch with params uses --json."""
        info = get_automation("scheduled_automation")
        assert info is not None

        spec = render_automation_workflow(info)

        job = spec.jobs["scheduled_automation"]
        # First dispatch has params (repo="main")
        dispatch_step = [s for s in job.steps if "build_flow" in s.name][0]
        assert "--json" in (dispatch_step.run or "")
        assert "repo" in (dispatch_step.run or "")

    def test_yaml_is_valid(self) -> None:
        """Test that generated YAML is valid."""
        info = get_automation("scheduled_automation")
        assert info is not None

        spec = render_automation_workflow(info)
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)
        assert parsed["name"] == "scheduled_automation"
        assert "schedule" in parsed["on"]


class TestAutomationRegistry:
    """Tests for automation registry."""

    def test_get_automation_by_short_name(self) -> None:
        """Test getting automation by short name."""
        info = get_automation("simple_automation")
        assert info is not None
        assert info.name == "simple_automation"

    def test_get_automation_not_found(self) -> None:
        """Test getting non-existent automation."""
        info = get_automation("nonexistent")
        assert info is None

    def test_registry_contains_all(self) -> None:
        """Test registry contains all automations."""
        registry = get_automation_registry()
        names = [info.name for info in registry.values()]
        assert "simple_automation" in names
        assert "scheduled_automation" in names
        assert "push_automation" in names
