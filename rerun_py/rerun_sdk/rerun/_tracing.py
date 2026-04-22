"""
OpenTelemetry tracing helper shared across Rerun-based Python code.

Provides [`with_tracing`][rerun._tracing.with_tracing], a decorator that creates
a Python OpenTelemetry span and bridges the trace context into Rerun's Rust SDK
so the `#[instrument]` spans on the Rust side become children of the Python span.

Active only when `TELEMETRY_ENABLED=true` and `OTEL_SDK_ENABLED=true` are set in
the environment. Otherwise the decorator is a pass-through.

This module is private — external callers should re-export `with_tracing` from
the consumer package rather than importing `rerun._tracing` directly.
"""

from __future__ import annotations

import functools
import logging
import os
from collections.abc import Callable
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


def _push_carrier_to_rust() -> tuple[Any, Any] | None:
    """
    Write the current OTel context into Rerun's `ContextVar` so Rust can read it.

    Returns `(context_var, token)` to pass to [`_pop_carrier_from_rust`][], or
    `None` if Rust was built without `perf_telemetry` or no OTel context is active.
    """
    from opentelemetry import context
    from opentelemetry.propagate import get_global_textmap

    from rerun.catalog import get_trace_context_var

    trace_ctx = get_trace_context_var()
    if trace_ctx is None:
        return None

    carrier: dict[str, str] = {}
    get_global_textmap().inject(carrier, context.get_current())
    if not carrier:
        return None

    return trace_ctx, trace_ctx.set(carrier)


def _pop_carrier_from_rust(attachment: tuple[Any, Any] | None) -> None:
    if attachment is None:
        return
    trace_ctx, token = attachment
    trace_ctx.reset(token)


def with_tracing(name: str) -> Callable[[F], F]:
    """
    Wrap a function in an OpenTelemetry span and propagate trace context into Rerun's Rust SDK.

    When enabled, creates a span named `name`, injects the W3C `traceparent` header into
    Rerun's shared `ContextVar`, and runs the wrapped function. Any Rust-side
    `#[instrument]` spans triggered from within (e.g. catalog queries) will be
    parented under this span in Jaeger.

    No-op unless `TELEMETRY_ENABLED=true` and `OTEL_SDK_ENABLED=true`.
    """

    def decorator(func: F) -> F:
        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            _init_once()
            if not _enabled:
                return func(*args, **kwargs)

            from opentelemetry import trace

            with trace.get_tracer("rerun").start_as_current_span(name):
                attachment = _push_carrier_to_rust()
                try:
                    return func(*args, **kwargs)
                finally:
                    _pop_carrier_from_rust(attachment)

        return cast("F", wrapper)

    return decorator
