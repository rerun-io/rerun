# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/views/dataframe.fbs".

from __future__ import annotations

from collections.abc import Iterable, Mapping

from ..._baseclasses import (
    DescribedComponentBatch,
)

__all__ = ["DataframeView"]


from ... import datatypes
from ..._baseclasses import AsComponents
from ...datatypes import EntityPathLike, Utf8Like
from .. import archetypes as blueprint_archetypes
from ..api import View, ViewContentsLike


class DataframeView(View):
    """
    **View**: A view to display any data in a tabular form.

    Any data from the store can be shown, using a flexibly, user-configurable query.

    ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**

    Example
    -------
    ### Use a blueprint to customize a DataframeView.:
    ```python
    import math

    import rerun as rr
    import rerun.blueprint as rrb

    rr.init("rerun_example_dataframe", spawn=True)

    # Log some data.
    for t in range(int(math.pi * 4 * 100.0)):
        rr.set_time("t", duration=t)
        rr.log("trig/sin", rr.Scalars(math.sin(float(t) / 100.0)))
        rr.log("trig/cos", rr.Scalars(math.cos(float(t) / 100.0)))

        # some sparse data
        if t % 5 == 0:
            rr.log("trig/tan_sparse", rr.Scalars(math.tan(float(t) / 100.0)))

    # Create a Dataframe View
    blueprint = rrb.Blueprint(
        rrb.DataframeView(
            origin="/trig",
            query=rrb.archetypes.DataframeQuery(
                timeline="t",
                filter_by_range=(rr.TimeInt(seconds=0), rr.TimeInt(seconds=20)),
                filter_is_not_null="/trig/tan_sparse:Scalar",
                select=["t", "log_tick", "/trig/sin:Scalar", "/trig/cos:Scalar", "/trig/tan_sparse:Scalar"],
            ),
        ),
    )

    rr.send_blueprint(blueprint)
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/dataframe_view/f89ae330b04baaa9b7576765dce37b5d4e7cef4e/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/dataframe_view/f89ae330b04baaa9b7576765dce37b5d4e7cef4e/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dataframe_view/f89ae330b04baaa9b7576765dce37b5d4e7cef4e/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dataframe_view/f89ae330b04baaa9b7576765dce37b5d4e7cef4e/1200w.png">
      <img src="https://static.rerun.io/dataframe_view/f89ae330b04baaa9b7576765dce37b5d4e7cef4e/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self,
        *,
        origin: EntityPathLike = "/",
        contents: ViewContentsLike = "$origin/**",
        name: Utf8Like | None = None,
        visible: datatypes.BoolLike | None = None,
        defaults: Iterable[AsComponents | Iterable[DescribedComponentBatch]] | None = None,
        overrides: Mapping[
            EntityPathLike,
            AsComponents | Iterable[DescribedComponentBatch | AsComponents | Iterable[DescribedComponentBatch]],
        ]
        | None = None,
        query: blueprint_archetypes.DataframeQuery | None = None,
    ) -> None:
        """
        Construct a blueprint for a new DataframeView view.

        Parameters
        ----------
        origin:
            The `EntityPath` to use as the origin of this view.
            All other entities will be transformed to be displayed relative to this origin.
        contents:
            The contents of the view specified as a query expression.
            This is either a single expression, or a list of multiple expressions.
            See [rerun.blueprint.archetypes.ViewContents][].
        name:
            The display name of the view.
        visible:
            Whether this view is visible.

            Defaults to true if not specified.
        defaults:
            List of archetypes or (described) component batches to add to the view.
            When an archetype in the view is missing a component included in this set,
            the value of default will be used instead of the normal fallback for the visualizer.

            Note that an archetype's required components typically don't have any effect.
            It is recommended to use the archetype's `from_fields` method instead and only specify the fields that you need.
        overrides:
            Dictionary of overrides to apply to the view. The key is the path to the entity where the override
            should be applied. The value is a list of archetypes or (described) component batches to apply to the entity.

            It is recommended to use the archetype's `from_fields` method instead and only specify the fields that you need.

            Important note: the path must be a fully qualified entity path starting at the root. The override paths
            do not yet support `$origin` relative paths or glob expressions.
            This will be addressed in <https://github.com/rerun-io/rerun/issues/6673>.

        query:
            Query of the dataframe.

        """

        properties: dict[str, AsComponents] = {}
        if query is not None:
            if not isinstance(query, blueprint_archetypes.DataframeQuery):
                query = blueprint_archetypes.DataframeQuery(query)
            properties["DataframeQuery"] = query

        super().__init__(
            class_identifier="Dataframe",
            origin=origin,
            contents=contents,
            name=name,
            visible=visible,
            properties=properties,
            defaults=defaults,
            overrides=overrides,
        )
