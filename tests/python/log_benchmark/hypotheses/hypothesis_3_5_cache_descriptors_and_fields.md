# Hypothesis 3+5: cache ComponentDescriptors and pre-compute component fields

## Hypothesis
H3: `as_component_batches()` creates new `ComponentDescriptor` Rust objects every call. These can be cached per (archetype_class, field_name, component_type) tuple.

H5: `as_component_batches()` iterates all 8 fields checking `"component" in fld.metadata` for each. Pre-computing the list of component field names avoids iterating non-component fields and checking metadata dicts.

## Code changes
In `_baseclasses.py`:
1. Added `_get_component_field_names()` classmethod that caches the list of component field names per archetype class
2. Added `_get_descriptor_cache()` classmethod that returns a per-class dict mapping `(field_name, component_type)` to `ComponentDescriptor`
3. `as_component_batches()` now uses cached field names and descriptors

## Results (release build, 100 entities x 1000 time steps, cumulative with H1+H2)

| Metric | H1+H2 only | + H3+H5 | Change |
|--------|------------|---------|--------|
| Full log | 40.5k xforms/s | 42.6k xforms/s | +5% |

## Decision: KEEP
~5% improvement from descriptor/field caching. Zero risk — caches are per-class and immutable after first population.
