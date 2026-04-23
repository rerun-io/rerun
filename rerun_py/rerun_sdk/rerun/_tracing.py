"""
OpenTelemetry tracing helpers shared across Rerun-based Python code.

Provides [`with_tracing`][rerun._tracing.with_tracing] (decorator) and
[`tracing_scope`][rerun._tracing.tracing_scope] (context manager), which both
create a Python OpenTelemetry span and bridge the trace context into Rerun's
Rust SDK so `#[instrument]` spans on the Rust side become children of the
Python span.

Active only when `TELEMETRY_ENABLED=true` and `OTEL_SDK_ENABLED=true` are set in
the environment. Otherwise both helpers are pass-throughs.

This module is private — external callers should re-export these helpers from
the consumer package rather than importing `rerun._tracing` directly.
"""

from __future__ import annotations

import contextlib
import functools
import logging
import os
from collections.abc import Callable, Iterator
from typing import Any, TypeVar, cast

logger = logging.getLogger(__name__)

F = TypeVar("F", bound=Callable[..., Any])

_TRUTHY = {"1", "true", "yes", "on"}


def _env_bool(name: str) -> bool:
    return (os.environ.get(name) or "").lower() in _TRUTHY


_initialized = False
_enabled = False


def _init_once() -> None:
    """Set up the Python OTel tracer provider the first time it is needed."""
    global _initialized, _enabled
    if _initialized:
        return
    _initialized = True

    if not _env_bool("TELEMETRY_ENABLED") or not _env_bool("OTEL_SDK_ENABLED"):
        return

    try:
        from opentelemetry import trace
        from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
        from opentelemetry.sdk.resources import Resource
        from opentelemetry.sdk.trace import TracerProvider
        from opentelemetry.sdk.trace.export import BatchSpanProcessor
    except ImportError:
        logger.warning("`with_tracing` is a no-op: install OpenTelemetry via `pip install rerun-sdk[tracing]`")
        return

    service_name = os.environ.get("OTEL_SERVICE_NAME") or "rerun-py"
    endpoint = os.environ.get("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") or "http://localhost:4317"

    provider = TracerProvider(resource=Resource.create({"service.name": service_name}))
    provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter(endpoint=endpoint)))
    trace.set_tracer_provider(provider)

    _enabled = True
    logger.info("Python OpenTelemetry tracing enabled (service=%s, endpoint=%s)", service_name, endpoint)


def current_trace_carrier() -> dict[str, str] | None:
    """
    Capture the current OTel context as a W3C carrier dict.

    Returns `None` if tracing is disabled or no context is active. The carrier
    is safe to pickle and pass across process boundaries; pair with
    [`attach_parent_carrier`][rerun._tracing.attach_parent_carrier] in the child
    to make its spans children of the captured context.
    """
    _init_once()
    if not _enabled:
        return None

    from opentelemetry import context
    from opentelemetry.propagate import get_global_textmap

    carrier: dict[str, str] = {}
    get_global_textmap().inject(carrier, context.get_current())
    return carrier or None


def attach_parent_carrier(carrier: dict[str, str] | None) -> None:
    """
    Attach a W3C carrier as the ambient parent context for the current thread.

    All subsequent spans created on this thread (via [`with_tracing`][rerun._tracing.with_tracing]
    or otherwise) will be parented under the extracted context. The attachment
    is intentionally not detached — call this once per worker thread/process on
    entry, typically from `__setstate__` after unpickling.

    No-op when `carrier` is `None` or tracing is disabled.
    """
    if not carrier:
        return
    _init_once()
    if not _enabled:
        return

    from opentelemetry import context
    from opentelemetry.propagate import get_global_textmap

    context.attach(get_global_textmap().extract(carrier))


def _push_carrier_to_rust() -> tuple[Any, Any] | None:
    """
    Write the current OTel context into Rerun's `ContextVar` so Rust can read it.

    Returns `(context_var, token)` to pass to [`_pop_carrier_from_rust`][], or
    `None` if Rust was built without `perf_telemetry` or no OTel context is active.
    """
    from rerun_bindings import _get_trace_context_var

    trace_ctx = _get_trace_context_var()
    if trace_ctx is None:
        return None

    carrier = current_trace_carrier()
    if carrier is None:
        return None

    return trace_ctx, trace_ctx.set(carrier)


def _pop_carrier_from_rust(attachment: tuple[Any, Any] | None) -> None:
    if attachment is None:
        return
    trace_ctx, token = attachment
    trace_ctx.reset(token)


@contextlib.contextmanager
def tracing_scope(name: str) -> Iterator[None]:
    """
    Open an OpenTelemetry span for the duration of a `with` block and propagate trace context into Rerun's Rust SDK.

    Context-manager counterpart to [`with_tracing`][rerun._tracing.with_tracing] —
    use it to scope arbitrary blocks of code without extracting them into a
    function. Any Rust-side `#[instrument]` spans triggered from within will be
    parented under this span in Jaeger.

    No-op unless `TELEMETRY_ENABLED=true` and `OTEL_SDK_ENABLED=true`.

    Examples
    --------
    ```python
    for epoch in range(num_epochs):
        with tracing_scope(f"epoch {epoch}"):
            train_one_epoch(...)
    ```

    """
    _init_once()
    if not _enabled:
        yield
        return

    from opentelemetry import trace

    with trace.get_tracer("rerun").start_as_current_span(name):
        attachment = _push_carrier_to_rust()
        try:
            yield
        finally:
            _pop_carrier_from_rust(attachment)


def with_tracing(name: str) -> Callable[[F], F]:
    """
    Wrap a function in an OpenTelemetry span and propagate trace context into Rerun's Rust SDK.

    When enabled, creates a span named `name`, injects the W3C `traceparent` header into
    Rerun's shared `ContextVar`, and runs the wrapped function. Any Rust-side
    `#[instrument]` spans triggered from within (e.g. catalog queries) will be
    parented under this span in Jaeger.

    For ad-hoc blocks that don't belong in a dedicated function, use
    [`tracing_scope`][rerun._tracing.tracing_scope] instead.

    No-op unless `TELEMETRY_ENABLED=true` and `OTEL_SDK_ENABLED=true`.
    """

    def decorator(func: F) -> F:
        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            with tracing_scope(name):
                return func(*args, **kwargs)

        return cast("F", wrapper)

    return decorator
