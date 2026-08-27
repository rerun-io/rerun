// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Name of the map provider to be used in Map views.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "stable")]
pub enum MapProvider {
    /// `OpenStreetMap` is the default map provider.
    #[default]
    OpenStreetMap = 1,

    /// Mapbox Streets is a minimalistic map designed by Mapbox.
    MapboxStreets = 2,

    /// Mapbox Dark is a dark-themed map designed by Mapbox.
    MapboxDark = 3,

    /// Mapbox Satellite is a satellite map designed by Mapbox.
    MapboxSatellite = 4,

    /// Mapbox Light is a light-themed map designed by Mapbox.
    MapboxLight = 5,
}
