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
    from .flowgraph import TaskNode

P = ParamSpec("P")
T = TypeVar("T")


class TaskWrapper(Protocol[T]):
    """
    Protocol describing a task-decorated function.

    Task wrappers are callable (returning Result[T]) and have a .flow() method
    for use in declarative flows (returning TaskNode[T]).
    """

    _task_info: TaskInfo

    def __call__(self, **kwargs: Any) -> Result[T]: ...

    def flow(self, **kwargs: Any) -> TaskNode[T]: ...


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

    @property
    def full_name(self) -> str:
        """Full qualified name of the task."""
        return f"{self.module}:{self.name}"


# Global registry of all tasks
_task_registry: dict[str, TaskInfo] = {}


def get_registry() -> dict[str, TaskInfo]:
    """Get the task registry."""
    return _task_registry


def get_task(name: str) -> TaskInfo | None:
    """Get a task by name. Tries full name first, then short name."""
    # Try exact match first
    if name in _task_registry:
        return _task_registry[name]

    # Try matching by short name
    for full_name, info in _task_registry.items():
        if info.name == name:
            return info

    return None


def _is_method_signature(fn: Callable[..., Any]) -> bool:
    """Check if a function signature indicates it's a method (first param is 'self')."""
    sig = inspect.signature(fn)
    params = list(sig.parameters.keys())
    return len(params) > 0 and params[0] == "self"


def task(fn: Callable[P, Result[T]]) -> TaskWrapper[T]:
    """
    Decorator to mark a function as a recompose task.

    The decorated function:
    - Is registered in the global task registry
    - Gets automatic context management
    - Has exceptions caught and converted to Err results
    - Can still be called as a normal Python function
    - Has a .flow() method for declarative flow building

    For methods (functions with 'self' as first parameter):
    - The method is marked but NOT registered immediately
    - Use @taskclass on the class to complete registration

    Usage:
        @task
        def compile(*, source: Path) -> Result[Path]:
            ...

        # Direct execution:
        result = compile(source=Path("src/"))  # Returns Result[Path]

        # Inside a declarative flow:
        node = compile.flow(source=Path("src/"))  # Returns TaskNode[Path]
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
        from .flow import DirectTaskCallInFlowError, get_current_plan

        # Check if we're inside a flow that's building a plan
        # If so, the user should use .flow() instead of direct call
        if get_current_plan() is not None:
            raise DirectTaskCallInFlowError(info.name)

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
    _task_registry[info.full_name] = info

    # Attach task info to wrapper for introspection
    wrapper._task_info = info  # type: ignore[attr-defined]

    # Add .flow() method for declarative flow building
    def flow_variant(**kwargs: Any) -> Any:
        """
        Create a TaskNode for this task (for use in declarative flows).

        This method can only be called inside a @flow-decorated function.
        It returns a TaskNode that represents a deferred execution of this task.

        Args:
            **kwargs: The arguments to pass to the task when executed.
                      May include TaskNode values from other .flow() calls.

        Returns:
            TaskNode[T] representing this task in the flow graph.

        Raises:
            RuntimeError: If called outside a @flow context.
        """
        from .flow import get_current_plan
        from .flowgraph import TaskNode

        plan = get_current_plan()
        if plan is None:
            raise RuntimeError(
                f"{info.name}.flow() can only be called inside a @flow-decorated function. "
                f"Use {info.name}() for direct execution."
            )

        # Create the TaskNode
        node: TaskNode[T] = TaskNode(task_info=info, kwargs=kwargs)
        plan.add_node(node)
        return node

    wrapper.flow = flow_variant  # type: ignore[attr-defined]

    # Cast to TaskWrapper to satisfy type checker
    # (we've added .flow and ._task_info attributes dynamically)
    from typing import cast

    return cast(TaskWrapper[T], wrapper)


def taskclass(cls: type[T]) -> type[T]:
    """
    Decorator to register a class with @task-decorated methods.

    This scans the class for methods decorated with @task and registers them
    as class-based tasks. The CLI will expose them as `classname.methodname`
    commands, combining __init__ arguments with method arguments.

    Example:
        @recompose.taskclass
        class Venv:
            def __init__(self, *, location: Path):
                self.location = location

            @recompose.task
            def sync(self, *, group: str | None = None) -> recompose.Result[None]:
                ...

        # CLI: ./app.py venv.sync --location=/tmp/venv --group=dev
    """
    class_name = cls.__name__.lower()
    module = cls.__module__

    # Get __init__ parameters (excluding 'self')
    init_sig = inspect.signature(cls.__init__)
    init_params = [
        p for name, p in init_sig.parameters.items()
        if name != "self"
    ]

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
        method_params = [
            p for name, p in method_sig.parameters.items()
            if name != "self"
        ]

        # Build combined signature: init params + method params
        combined_params = init_params + method_params
        combined_sig = inspect.Signature(parameters=combined_params)

        # Task name: classname.methodname
        task_name = f"{class_name}.{attr_name}"

        # Create wrapper that constructs instance and calls method
        def make_wrapper(
            cls: type, method_name: str, init_param_names: list[str], full_task_name: str
        ) -> Callable[..., Any]:
            """Create a wrapper for a specific method."""
            def wrapper(**kwargs: Any) -> Result[Any]:
                from .flow import DirectTaskCallInFlowError, get_current_plan

                # Check if we're inside a flow that's building a plan
                if get_current_plan() is not None:
                    raise DirectTaskCallInFlowError(full_task_name)

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
        wrapper = make_wrapper(cls, attr_name, init_param_names, task_name)
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

        _task_registry[info.full_name] = info

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
