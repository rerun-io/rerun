"""Task decorator and registry for recompose."""

from __future__ import annotations

import functools
import inspect
import traceback
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Generic, ParamSpec, Protocol, TypeVar

from .context import Context, get_context, set_context
from .result import Err, Result

if TYPE_CHECKING:
    from .plan import TaskClassNode, TaskNode

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

        plan = get_current_plan()

        if plan is not None:
            # FLOW-BUILDING MODE: Create TaskNode and add to plan
            _validate_task_kwargs(info.name, info.signature, kwargs)
            node = _create_task_node(info, kwargs)
            plan.add_node(node)
            return node  # type: ignore[return-value]

        # NORMAL EXECUTION MODE
        return _run_with_context(info.name, fn, args, kwargs)

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


@dataclass
class TaskClassInfo:
    """Metadata about a TaskClass."""

    name: str
    """Lowercase class name."""

    module: str
    """Module where the class is defined."""

    cls: type
    """The original class."""

    init_signature: inspect.Signature
    """Signature of __init__ (excluding self)."""

    method_tasks: dict[str, TaskInfo]
    """Map of method name -> TaskInfo for @task methods."""


def taskclass(cls: type[T]) -> type[T]:
    """
    Decorator to register a class as a TaskClass.

    A TaskClass can be used in two modes:

    1. **Direct mode** (outside flows): Creates a normal instance, @task methods
       execute immediately when called.

    2. **Flow mode** (inside @flow): Instantiation returns a TaskClassNode.
       Method calls on the TaskClassNode create TaskNodes in the flow plan.

    TaskClasses support:
    - `__init__` becomes a task step (no decoration needed)
    - `@task` decorated methods become task steps
    - Non-decorated methods are regular methods (usable when passed to other tasks)

    Example:
        @recompose.taskclass
        class Venv:
            def __init__(self, *, location: Path):
                self.location = location
                # Create venv...

            @recompose.task
            def install_wheel(self, *, wheel: str) -> recompose.Result[None]:
                # Install wheel...

            def run(self, *args: str) -> recompose.RunResult:
                # Regular method - run command in venv
                python = self.location / "bin" / "python"
                return recompose.run(str(python), *args)

        # In a flow:
        @recompose.flow
        def wheel_test() -> None:
            venv = Venv(location=Path("/tmp/test"))  # TaskClassNode
            venv.install_wheel(wheel="pkg.whl")       # TaskNode
            smoke_test(venv=venv)                     # Depends on install_wheel

        # Direct usage:
        venv = Venv(location=Path("/tmp/test"))  # Actual Venv instance
        venv.install_wheel(wheel="pkg.whl")       # Executes immediately

    """
    from .flow import get_current_plan
    from .plan import TaskClassNode

    class_name = cls.__name__.lower()
    module = cls.__module__

    # Get __init__ parameters (excluding 'self')
    init_sig = inspect.signature(cls.__init__)
    init_params = [p for name, p in init_sig.parameters.items() if name != "self"]
    init_param_names = [p.name for p in init_params]

    # Collect @task-decorated methods
    method_tasks: dict[str, TaskInfo] = {}

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
        method_param_sig = inspect.Signature(parameters=method_params)

        # Task name for the method: classname.methodname
        task_name = f"{class_name}.{attr_name}"

        # Create TaskInfo for method (fn will be set later when we have an instance)
        info = TaskInfo(
            name=task_name,
            module=module,
            fn=method,  # Will be replaced with bound method at runtime
            original_fn=method,
            signature=method_param_sig,
            doc=method_doc,
            cls=cls,
            is_method=True,
            method_name=attr_name,
            init_params=init_params,
        )

        method_tasks[attr_name] = info

    # Create TaskInfo for __init__
    init_task_name = f"{class_name}.__init__"
    init_param_sig = inspect.Signature(parameters=init_params)

    init_task_info = TaskInfo(
        name=init_task_name,
        module=module,
        fn=cls.__init__,
        original_fn=cls.__init__,
        signature=init_param_sig,
        doc=cls.__init__.__doc__ or cls.__doc__,
        cls=cls,
        is_method=True,
        method_name="__init__",
    )

    # Store class info
    taskclass_info = TaskClassInfo(
        name=class_name,
        module=module,
        cls=cls,
        init_signature=init_param_sig,
        method_tasks=method_tasks,
    )

    # Store on class for introspection
    cls._taskclass_info = taskclass_info  # type: ignore[attr-defined]
    cls._init_task_info = init_task_info  # type: ignore[attr-defined]

    # Save the original __new__ and __init__
    original_new = cls.__new__
    original_init = cls.__init__

    def new_wrapper(wrapped_cls: type[T], *args: Any, **kwargs: Any) -> T | TaskClassNode[T]:
        """
        Intercept instantiation to detect flow context.

        In flow context: Return a TaskClassNode
        Otherwise: Return a normal instance
        """
        plan = get_current_plan()

        if plan is not None:
            # FLOW-BUILDING MODE: Create TaskClassNode
            _validate_task_kwargs(init_task_name, init_param_sig, kwargs)

            # Create init TaskNode
            from .conditional import get_current_condition

            current_cond = get_current_condition()
            condition = current_cond.condition if current_cond else None

            from .plan import TaskNode

            init_node: TaskNode[T] = TaskNode(
                task_info=init_task_info,
                kwargs=kwargs.copy(),
                condition=condition,
            )
            plan.add_node(init_node)

            # Create TaskClassNode
            taskclass_node: TaskClassNode[T] = TaskClassNode(
                cls=wrapped_cls,
                init_kwargs=kwargs.copy(),
                init_node=init_node,
                current_node=init_node,
            )

            # Return a proxy that intercepts method calls
            return _TaskClassNodeProxy(taskclass_node, method_tasks)  # type: ignore[return-value]

        # NORMAL EXECUTION MODE: Create actual instance
        if original_new is object.__new__:
            instance = object.__new__(wrapped_cls)
        else:
            instance = original_new(wrapped_cls)

        return instance

    # Replace __new__
    cls.__new__ = new_wrapper  # type: ignore[method-assign]

    # Also create flat wrappers for CLI registration (backward compatibility)
    task_wrappers: dict[str, Any] = {}

    for method_name, method_info in method_tasks.items():
        # Build combined signature: init params + method params
        combined_params = init_params + list(method_info.signature.parameters.values())
        combined_sig = inspect.Signature(parameters=combined_params)
        task_name = method_info.name

        def make_flat_wrapper(
            cls: type,
            method_name: str,
            init_param_names: list[str],
            full_task_name: str,
            task_sig: inspect.Signature,
            method_info: TaskInfo,
        ) -> Callable[..., Any]:
            """Create a flat wrapper for CLI registration."""

            def wrapper(**kwargs: Any) -> Result[Any]:
                plan = get_current_plan()

                if plan is not None:
                    # FLOW-BUILDING MODE: Create TaskNode and add to plan
                    _validate_task_kwargs(full_task_name, task_sig, kwargs)
                    node = _create_task_node(wrapper._task_info, kwargs)  # type: ignore[attr-defined]
                    plan.add_node(node)
                    return node  # type: ignore[return-value]

                # NORMAL EXECUTION MODE
                init_kwargs = {k: v for k, v in kwargs.items() if k in init_param_names}
                method_kwargs = {k: v for k, v in kwargs.items() if k not in init_param_names}

                instance = cls(**init_kwargs)
                bound_method = getattr(instance, method_name)

                return _run_with_context(full_task_name, bound_method, (), method_kwargs)

            return wrapper

        wrapper = make_flat_wrapper(cls, method_name, init_param_names, task_name, combined_sig, method_info)
        wrapper.__doc__ = method_info.doc

        # Create combined TaskInfo for flat wrapper
        flat_info = TaskInfo(
            name=task_name,
            module=module,
            fn=wrapper,
            original_fn=method_info.original_fn,
            signature=combined_sig,
            doc=method_info.doc,
            cls=cls,
            is_method=True,
            method_name=method_name,
            init_params=init_params,
        )
        wrapper._task_info = flat_info  # type: ignore[attr-defined]
        task_wrappers[method_name] = wrapper

    cls._recompose_tasks = task_wrappers  # type: ignore[attr-defined]

    return cls


class _TaskClassNodeProxy(Generic[T]):
    """
    Proxy that wraps a TaskClassNode and intercepts method calls.

    When a @task method is called on this proxy, it creates a TaskNode
    and updates the TaskClassNode's current_node for dependency tracking.

    For non-task methods, it raises an error (they should only be called
    on actual instances, not in flow context).
    """

    def __init__(self, taskclass_node: TaskClassNode[T], method_tasks: dict[str, TaskInfo]):
        # Use object.__setattr__ to avoid triggering __setattr__ override
        object.__setattr__(self, "_taskclass_node", taskclass_node)
        object.__setattr__(self, "_method_tasks", method_tasks)

    def __getattr__(self, name: str) -> Any:
        taskclass_node: TaskClassNode[T] = object.__getattribute__(self, "_taskclass_node")
        method_tasks: dict[str, TaskInfo] = object.__getattribute__(self, "_method_tasks")

        if name in method_tasks:
            # Return a callable that creates a TaskNode when called
            method_info = method_tasks[name]
            return _TaskMethodCaller(taskclass_node, method_info)

        # For non-task attributes, raise an error - they're not available in flow context
        raise AttributeError(
            f"Cannot access '{name}' on TaskClassNode in flow context. "
            f"Only @task-decorated methods can be called in flows. "
            f"Available task methods: {list(method_tasks.keys())}"
        )

    @property
    def _is_taskclass_node_proxy(self) -> bool:
        """Marker to identify this as a TaskClassNode proxy."""
        return True

    @property
    def node(self) -> TaskClassNode[T]:
        """Get the underlying TaskClassNode."""
        return object.__getattribute__(self, "_taskclass_node")

    def __repr__(self) -> str:
        taskclass_node: TaskClassNode[T] = object.__getattribute__(self, "_taskclass_node")
        return f"TaskClassNodeProxy({taskclass_node!r})"


class _TaskMethodCaller(Generic[T]):
    """
    Callable that creates a TaskNode when invoked.

    This is returned when accessing a @task method on a TaskClassNodeProxy.
    """

    def __init__(self, taskclass_node: TaskClassNode[T], method_info: TaskInfo):
        self._taskclass_node = taskclass_node
        self._method_info = method_info

    def __call__(self, **kwargs: Any) -> Result[Any]:
        from .conditional import get_current_condition
        from .flow import get_current_plan
        from .plan import TaskNode

        plan = get_current_plan()
        if plan is None:
            raise RuntimeError("_TaskMethodCaller should only be used in flow context")

        # Validate kwargs
        _validate_task_kwargs(self._method_info.name, self._method_info.signature, kwargs)

        # Get current condition
        current_cond = get_current_condition()
        condition = current_cond.condition if current_cond else None

        # Get the current dependency node BEFORE we update it
        prev_node = self._taskclass_node.current_node

        # Create kwargs that includes reference to the TaskClassNode
        full_kwargs = kwargs.copy()
        full_kwargs["__taskclass_node__"] = self._taskclass_node

        # Create TaskNode for this method call with explicit dependency
        node: TaskNode[Any] = TaskNode(
            task_info=self._method_info,
            kwargs=full_kwargs,
            condition=condition,
            taskclass_dep=prev_node,  # Explicit dependency on previous node
        )

        plan.add_node(node)

        # Update the TaskClassNode's current_node AFTER creating the node
        self._taskclass_node.current_node = node

        return node  # type: ignore[return-value]


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


def _validate_task_kwargs(task_name: str, sig: inspect.Signature, kwargs: dict[str, Any]) -> None:
    """Validate kwargs against task signature. Raises TypeError if invalid."""
    valid_params = set(sig.parameters.keys())
    unexpected = set(kwargs.keys()) - valid_params
    if unexpected:
        raise TypeError(
            f"{task_name}() got unexpected keyword argument(s): {', '.join(sorted(unexpected))}. "
            f"Valid arguments are: {', '.join(sorted(valid_params))}"
        )

    missing = []
    for name, param in sig.parameters.items():
        if param.default is inspect.Parameter.empty and name not in kwargs:
            missing.append(name)
    if missing:
        raise TypeError(f"{task_name}() missing required keyword argument(s): {', '.join(missing)}")


def _create_task_node(info: TaskInfo, kwargs: dict[str, Any]) -> TaskNode[Any]:
    """Create a TaskNode for flow-building mode."""
    from .conditional import get_current_condition
    from .plan import TaskNode

    current_cond = get_current_condition()
    condition = current_cond.condition if current_cond else None
    return TaskNode(task_info=info, kwargs=kwargs, condition=condition)


def _run_with_context(
    task_name: str, fn: Callable[..., Any], args: tuple[Any, ...], kwargs: dict[str, Any]
) -> Result[Any]:
    """Execute task with context management."""
    existing_ctx = get_context()

    if existing_ctx is None:
        ctx = Context(task_name=task_name)
        set_context(ctx)
        try:
            return _execute_task(fn, args, kwargs)
        finally:
            set_context(None)
    else:
        return _execute_task(fn, args, kwargs)
