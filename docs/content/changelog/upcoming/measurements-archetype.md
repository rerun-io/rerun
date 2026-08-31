---
title: "Measurements archetype"
hidden: true
type: feature
---

### Measurements archetype

The new `Measurements` archetype logs scalar values together with their uncertainty.
Use it for sensors that report a value and a variance: pressure, temperature, illuminance, relative humidity, range, and so on.

In a time series view each series is drawn as a line with a translucent band around it, one standard deviation (the square root of the variance) wide in each direction.
Leave `variances` unset for values whose uncertainty is unknown.

<picture>
  <img src="https://static.rerun.io/measurements/2388490ab2b487bb6c47a2be3e7d5e7aa17c08f3/full.png" alt="">
</picture>

Two new components come with it.
`Variance` holds σ², in the units of the value squared, where `0` means a perfectly known value and draws no band.
`Unit` holds a display-only unit such as `"Pa"` or `"lux"`, shown in the legend and in tooltips.

Unlike `Scalars`, this archetype carries its own styling, so values and style are logged in one call:

```python
rr.log(
    "pressure",
    rr.Measurements(values=pressures, variances=variances, units="Pa"),
)
```

Docs: ../reference/types/archetypes/measurements.md
Example: ../reference/types/archetypes/measurements.md#example
