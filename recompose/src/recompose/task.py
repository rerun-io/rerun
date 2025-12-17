"""Task decorator and registry for recompose."""

from __future__ import annotations

import functools
import inspect
import traceback
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, ParamSpec, Protocol, TypeVar

from .context import Context, get_context, set_context
from .result import Err, Result

if TYPE_CHECKING:
    from .plan import TaskNode

P = ParamSpec("P")
T = TypeVar("T")


class TaskWrapper(Protocol[P, T]):
    """
    Protocol describing a task-decorated function.

    Task wrappers are callable and automatically detect whether they're being
    called inside a flow-building context or for direct execution.

    The wrapper has the same parameter signature (P) in both modes and returns
    Result[T] to the type checker, enabling type-safe flow composition:

        @flow
        def my_flow():
            result = greet(name="World")  # Type: Result[str]
            echo(message=result.value())  # Type: str (from Result.value())

    At runtime when inside a @flow, the call actually returns a TaskNode[T]
    that mimics Result[T]. The TaskNode.value() method returns itself, allowing
    it to be passed as a dependency to other task calls. The receiving task
    validates that inputs are either literal values or Input[T] types (TaskNode
    or InputPlaceholder).
    """

    _task_info: TaskInfo

    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> Result[T]: ...


@dataclass
class TaskInfo:
    """Metadata about a registered task."""

    name: str
    module: str
    fn: Callable[..., Any]  # The wrapped function (with context/exception handling)
    original_fn: Callable[..., Any]  # The original unwrapped function
    signature: inspect.Signature
    doc: str | None

    # Class-based task fields
    cls: type | None = None  # The class this method belongs to
    is_method: bool = False  # True if this is a method task
    method_name: str | None = None  # Original method name (without class prefix)
    init_params: list[inspect.Parameter] = field(default_factory=list)  # __init__ params (excluding self)

    # GHA action fields (for virtual tasks that map to `uses:` steps)
    is_gha_action: bool = False  # True if this is a GHA virtual action
    gha_uses: str | None = None  # The action to use, e.g., "actions/checkout@v4"

    # Setup step field (for workspace initialization infrastructure)
    is_setup_step: bool = False  # True if this is the setup_workspace step

    # Condition check step (for run_if evaluation)
    is_condition_check: bool = False  # True if this evaluates a run_if condition

    @property
    def full_name(self) -> str:
        """Full qualified name of the task."""
        return f"{self.module}:{self.name}"


def _is_method_signature(fn: Callable[..., Any]) -> bool:
    """Check if a function signature indicates it's a method (first param is 'self')."""
    sig = inspect.signature(fn)
    params = list(sig.parameters.keys())
    return len(params) > 0 and params[0] == "self"


def task(fn: Callable[P, Result[T]]) -> TaskWrapper[P, T]:
    """
    Decorator to mark a function as a recompose task.

    The decorated function:
    - Gets automatic context management
    - Has exceptions caught and converted to Err results
    - Automatically detects if it's called inside a flow and behaves accordingly

    Note: Tasks are NOT automatically registered. To expose a task as a CLI
    command, include it in the `commands` parameter to `recompose.main()`.

    For methods (functions with 'self' as first parameter):
    - The method is marked but NOT wrapped immediately
    - Use @taskclass on the class to complete wrapping

    Usage:
        @task
        def compile(*, source: Path) -> Result[Path]:
            ...

        # Direct execution:
        result = compile(source=Path("src/"))  # Returns Result[Path]

        # Inside a declarative flow - automatic graph building:
        @flow
        def build_flow():
            compiled = compile(source=Path("src/"))  # Type: Result[Path], runtime: TaskNode
            test(binary=compiled.value())            # Type: Path

    When called inside a @flow, the task automatically returns a TaskNode (which
    mimics Result[T]) instead of executing. This enables type-safe composition
    via .value() while building the task graph.
    """
    # Check if this looks like a method
    if _is_method_signature(fn):
        # Mark as pending method task - will be registered by @taskclass
        fn._is_pending_method_task = True  # type: ignore[attr-defined]
        fn._method_doc = fn.__doc__  # type: ignore[attr-defined]
        return fn  # type: ignore[return-value]  # @taskclass will handle wrapping

    # Regular function task - register immediately
    @functools.wraps(fn)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> Result[T]:
        from .flow import get_current_plan

        # Check if we're inside a flow that's building a plan
        plan = get_current_plan()

        if plan is not None:
            # FLOW-BUILDING MODE: Create TaskNode and add to plan
            # Validate kwargs against the task signature
            valid_params = set(info.signature.parameters.keys())
            unexpected = set(kwargs.keys()) - valid_params
            if unexpected:
                raise TypeError(
                    f"{info.name}() got unexpected keyword argument(s): {', '.join(sorted(unexpected))}. "
                    f"Valid arguments are: {', '.join(sorted(valid_params))}"
                )

            # Check for missing required arguments
            missing = []
            for name, param in info.signature.parameters.items():
                if param.default is inspect.Parameter.empty and name not in kwargs:
                    missing.append(name)
            if missing:
                raise TypeError(f"{info.name}() missing required keyword argument(s): {', '.join(missing)}")

            # Create the TaskNode, capturing current condition if in a run_if block
            from .conditional import get_current_condition
            from .plan import TaskNode

            current_cond = get_current_condition()
            condition = current_cond.condition if current_cond else None

            node: TaskNode[T] = TaskNode(task_info=info, kwargs=kwargs, condition=condition)
            plan.add_node(node)
            return node  # type: ignore[return-value]

        # NORMAL EXECUTION MODE: Execute the task
        # Check if we're already in a context
        existing_ctx = get_context()

        if existing_ctx is None:
            # Create a new context for this task
            ctx = Context(task_name=info.name)
            set_context(ctx)
            try:
                result = _execute_task(fn, args, kwargs)
            finally:
                set_context(None)
        else:
            # Already in a context, just execute
            result = _execute_task(fn, args, kwargs)

        return result

    # Create task info with the wrapper
    info = TaskInfo(
        name=fn.__name__,
        module=fn.__module__,
        fn=wrapper,  # Store the wrapper
        original_fn=fn,  # Keep reference to original
        signature=inspect.signature(fn),
        doc=fn.__doc__,
    )

    # Attach task info to wrapper for introspection
    wrapper._task_info = info  # type: ignore[attr-defined]

    # Cast to TaskWrapper to satisfy type checker
    # (we've added ._task_info attribute dynamically)
    from typing import cast

    return cast(TaskWrapper[P, T], wrapper)


def taskclass(cls: type[T]) -> type[T]:
    """
    Decorator to register a class with @task-decorated methods.

    This scans the class for methods decorated with @task and creates
    task wrappers. The wrappers are stored on the class as `_recompose_tasks`
    dict (mapping method name to wrapper).

    Example:
        @recompose.taskclass
        class Venv:
            def __init__(self, *, location: Path):
                self.location = location

            @recompose.task
            def sync(self, *, group: str | None = None) -> recompose.Result[None]:
                ...

        # Access task wrappers for explicit registration:
        commands = [
            recompose.CommandGroup("Venv", list(Venv._recompose_tasks.values())),
        ]

        # CLI: ./app.py venv.sync --location=/tmp/venv --group=dev

    """
    class_name = cls.__name__.lower()
    module = cls.__module__

    # Dict to store task wrappers for explicit registration
    task_wrappers: dict[str, Any] = {}

    # Get __init__ parameters (excluding 'self')
    init_sig = inspect.signature(cls.__init__)
    init_params = [p for name, p in init_sig.parameters.items() if name != "self"]

    # Scan class for @task-decorated methods
    for attr_name in dir(cls):
        if attr_name.startswith("_"):
            continue

        attr = getattr(cls, attr_name)
        if not callable(attr):
            continue

        # Check if this method was marked by @task
        if not getattr(attr, "_is_pending_method_task", False):
            continue

        method = attr
        method_doc = getattr(method, "_method_doc", None)

        # Get method signature (excluding 'self')
        method_sig = inspect.signature(method)
        method_params = [p for name, p in method_sig.parameters.items() if name != "self"]

        # Build combined signature: init params + method params
        combined_params = init_params + method_params
        combined_sig = inspect.Signature(parameters=combined_params)

        # Task name: classname.methodname
        task_name = f"{class_name}.{attr_name}"

        # Create wrapper that constructs instance and calls method
        def make_wrapper(
            cls: type, method_name: str, init_param_names: list[str], full_task_name: str, task_sig: inspect.Signature
        ) -> Callable[..., Any]:
            """Create a wrapper for a specific method."""

            def wrapper(**kwargs: Any) -> Result[Any]:
                from .flow import get_current_plan

                # Check if we're inside a flow that's building a plan
                plan = get_current_plan()

                if plan is not None:
                    # FLOW-BUILDING MODE: Create TaskNode and add to plan
                    # Validate kwargs against the task signature
                    valid_params = set(task_sig.parameters.keys())
                    unexpected = set(kwargs.keys()) - valid_params
                    if unexpected:
                        raise TypeError(
                            f"{full_task_name}() got unexpected keyword argument(s): {', '.join(sorted(unexpected))}. "
                            f"Valid arguments are: {', '.join(sorted(valid_params))}"
                        )

                    # Check for missing required arguments
                    missing = []
                    for name, param in task_sig.parameters.items():
                        if param.default is inspect.Parameter.empty and name not in kwargs:
                            missing.append(name)
                    if missing:
                        missing_args = ", ".join(missing)
                        raise TypeError(f"{full_task_name}() missing required keyword argument(s): {missing_args}")

                    # Create the TaskNode, capturing current condition if in a run_if block
                    from .conditional import get_current_condition
                    from .plan import TaskNode

                    current_cond = get_current_condition()
                    condition = current_cond.condition if current_cond else None

                    # Note: We'll need the TaskInfo reference, which will be set after this wrapper is created
                    # For now, we'll need to pass it differently - let's store it on the wrapper
                    node: Any = TaskNode(task_info=wrapper._task_info, kwargs=kwargs, condition=condition)  # type: ignore[attr-defined]
                    plan.add_node(node)
                    return node  # type: ignore[no-any-return]

                # NORMAL EXECUTION MODE: Execute the task
                # Split kwargs into init args and method args
                init_kwargs = {k: v for k, v in kwargs.items() if k in init_param_names}
                method_kwargs = {k: v for k, v in kwargs.items() if k not in init_param_names}

                # Construct instance
                instance = cls(**init_kwargs)

                # Get the actual method from the instance
                bound_method = getattr(instance, method_name)

                # Check if we're already in a context
                existing_ctx = get_context()

                if existing_ctx is None:
                    ctx = Context(task_name=f"{cls.__name__.lower()}.{method_name}")
                    set_context(ctx)
                    try:
                        result = _execute_task(bound_method, (), method_kwargs)
                    finally:
                        set_context(None)
                else:
                    result = _execute_task(bound_method, (), method_kwargs)

                return result

            return wrapper

        init_param_names = [p.name for p in init_params]
        wrapper = make_wrapper(cls, attr_name, init_param_names, task_name, combined_sig)
        wrapper.__doc__ = method_doc

        # Create TaskInfo for this method task
        info = TaskInfo(
            name=task_name,
            module=module,
            fn=wrapper,
            original_fn=method,
            signature=combined_sig,
            doc=method_doc,
            cls=cls,
            is_method=True,
            method_name=attr_name,
            init_params=init_params,
        )

        # Attach task info to wrapper for introspection (needed for flow building)
        wrapper._task_info = info  # type: ignore[attr-defined]

        # Store wrapper for explicit registration
        task_wrappers[attr_name] = wrapper

    # Store wrappers on class for explicit registration
    cls._recompose_tasks = task_wrappers  # type: ignore[attr-defined]

    return cls


def _execute_task(fn: Callable[..., Any], args: tuple[Any, ...], kwargs: dict[str, Any]) -> Result[Any]:
    """Execute a task function, catching exceptions."""
    try:
        result = fn(*args, **kwargs)

        # Ensure the result is a Result type
        if not isinstance(result, Result):
            # If the function didn't return a Result, wrap it
            from .result import Ok

            return Ok(result)

        return result

    except Exception as e:
        # Catch any exception and convert to Err
        tb = traceback.format_exc()
        return Err(f"{type(e).__name__}: {e}", traceback=tb)
