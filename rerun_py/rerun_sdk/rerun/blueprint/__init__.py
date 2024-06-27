from __future__ import annotations

# =====================================
# API RE-EXPORTS
# Important: always us the `import _ as _` format to make it explicit to type-checkers that these are public APIs.
#
from ..datatypes import (  # Re-export time range types for better discoverability.
    TimeRange as TimeRange,
    TimeRangeBoundary as TimeRangeBoundary,
)
from . import (
    archetypes as archetypes,
    components as components,
    visualizers as visualizers,
)
from .api import (
    Blueprint as Blueprint,
    BlueprintLike as BlueprintLike,
    BlueprintPanel as BlueprintPanel,
    BlueprintPart as BlueprintPart,
    Container as Container,
    ContainerLike as ContainerLike,
    PanelState as PanelState,
    PanelStateLike as PanelStateLike,
    SelectionPanel as SelectionPanel,
    SpaceView as SpaceView,
    TimePanel as TimePanel,
    TopPanel as TopPanel,
)
from .archetypes import (
    Background as Background,
    PlotLegend as PlotLegend,
    ScalarAxis as ScalarAxis,
    TensorScalarMapping as TensorScalarMapping,
    TensorSliceSelection as TensorSliceSelection,
    # VisibleTimeRanges, # Don't expose this mono-archetype directly - one can always use the component instead!
    VisualBounds2D as VisualBounds2D,
)
from .components import (
    BackgroundKind as BackgroundKind,
    Corner2D as Corner2D,
    LockRangeDuringZoom as LockRangeDuringZoom,
    VisibleTimeRange as VisibleTimeRange,
    VisualizerOverrides as VisualizerOverrides,
)
from .containers import (
    Grid as Grid,
    Horizontal as Horizontal,
    Tabs as Tabs,
    Vertical as Vertical,
)
from .views import (
    BarChartView as BarChartView,
    Spatial2DView as Spatial2DView,
    Spatial3DView as Spatial3DView,
    TensorView as TensorView,
    TextDocumentView as TextDocumentView,
    TextLogView as TextLogView,
    TimeSeriesView as TimeSeriesView,
)
