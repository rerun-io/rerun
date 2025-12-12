from __future__ import annotations

import logging
import os
from typing import TYPE_CHECKING

import pytest
from opentelemetry import context, trace
from opentelemetry.exporter.otlp.proto.grpc.metric_exporter import OTLPMetricExporter
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.propagate import get_global_textmap
from opentelemetry.sdk.metrics import Meter, MeterProvider
from opentelemetry.sdk.metrics.export import PeriodicExportingMetricReader
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

if TYPE_CHECKING:
    from collections.abc import Iterator
    from typing import Any

logger = logging.getLogger(__name__)


class Telemetry:
    """A class to manage OpenTelemetry setup, implemented as a singleton (not thread-safe)."""

    _instance: Telemetry | None = None

    def __new__(cls) -> Telemetry:
        # not thread-safe
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self) -> None:
        # Initialize only if it's the first time
        if not hasattr(self, "_initialized"):
            self.meter: Meter | None = None
            self.meter_provider: MeterProvider | None = None
            self.tracer_provider: TracerProvider | None = None

            otel_metrics_endpoint: str | None = os.environ.get("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT")
            otel_traces_endpoint: str | None = os.environ.get("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
            svc_name = os.environ.get("OTEL_SERVICE_NAME", "e2e-redap-tests")
            print("Setting up telemetry")
            resource: Resource | None = None
            if otel_metrics_endpoint or otel_traces_endpoint:
                resource = Resource.create({"service.name": svc_name})

            if otel_metrics_endpoint:
                metric_reader = PeriodicExportingMetricReader(OTLPMetricExporter(endpoint=otel_metrics_endpoint))
                self.meter_provider = MeterProvider(resource=resource, metric_readers=[metric_reader])
                self.meter = self.meter_provider.get_meter(svc_name)

            if otel_traces_endpoint:
                trace_exporter = OTLPSpanExporter(endpoint=otel_traces_endpoint)
                span_processor = BatchSpanProcessor(trace_exporter)
                self.tracer_provider = TracerProvider(resource=resource)
                self.tracer_provider.add_span_processor(span_processor)

                # Set as global tracer provider so trace context works
                trace.set_tracer_provider(self.tracer_provider)
            self._initialized: bool = True

    def shutdown(self) -> None:
        """Shutdown the telemetry providers."""
        if self.meter_provider:
            print(f"FLUSHING meter provider {self.meter_provider}")
            _ = self.meter_provider.force_flush(timeout_millis=5000)  # Force export before shutdown
            self.meter_provider.shutdown()
        if self.tracer_provider:
            print(f"FLUSHING trace provider {self.tracer_provider}")
            _ = self.tracer_provider.force_flush(timeout_millis=5000)
            self.tracer_provider.shutdown()

        self.meter = None
        self.meter_provider = None
        self.tracer_provider = None
        self._initialized = False


@pytest.fixture(scope="session", name="telemetry")
def telemetry_fixture() -> Iterator[Telemetry]:
    """Set up OpenTelemetry for the test session."""
    telemetry_instance = Telemetry()

    yield telemetry_instance
    telemetry_instance.shutdown()


@pytest.fixture(scope="function", name="tracing")
def tracing_fixture(request: pytest.FixtureRequest, telemetry: Telemetry) -> Iterator[trace.Span]:  # noqa: ARG001
    """Decorator to add OpenTelemetry tracing to test functions."""

    tracer = trace.get_tracer(__name__)
    print(f"Got tracer {tracer}")

    span_name: str = request.node.name
    # strip the test_ prefix from the span name
    span_name = span_name.removeprefix("test_")

    with tracer.start_as_current_span(span_name) as span:
        span.set_attribute("test_name", span_name)
        print(f"Got span {span}")

        # Try rerun context propagation if available
        token = None
        trace_ctx = None
        try:
            from rerun.catalog import _rerun_trace_context

            trace_ctx = _rerun_trace_context()
            current_ctx = context.get_current()
            carrier: dict[Any, Any] = {}
            get_global_textmap().inject(carrier, current_ctx)
            if carrier:
                token = trace_ctx.set(carrier)
        except ImportError:
            pass

        try:
            yield span
        finally:
            if token and trace_ctx:
                try:
                    trace_ctx.reset(token)
                except (NameError, AttributeError):
                    pass
