use re_sdk_types::blueprint::components::MapProvider;
use re_viewer_context::ViewerContext;

use crate::datatype_uis::{VariantAvailable, VariantAvailableProvider};

pub struct MapProviderVariantAvailable;

impl VariantAvailableProvider<MapProvider> for MapProviderVariantAvailable {
    fn is_variant_enabled(ctx: &ViewerContext<'_>, variant: MapProvider) -> VariantAvailable {
        let map_box_available = if ctx
            .app_options()
            .mapbox_access_token()
            .is_some_and(|token| !token.is_empty())
        {
            VariantAvailable::Yes
        } else {
            VariantAvailable::No {
                reason_markdown: "A Mapbox access token is not available. You can set it in the \
                settings or using the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable."
                    .to_owned(),
            }
        };

        match variant {
            MapProvider::OpenStreetMap => VariantAvailable::Yes,

            MapProvider::MapboxStreets
            | MapProvider::MapboxDark
            | MapProvider::MapboxSatellite
            | MapProvider::MapboxLight => map_box_available,
        }
    }
}
