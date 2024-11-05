use re_types::blueprint::components::MapProvider;
use re_viewer_context::ViewerContext;

use crate::datatype_uis::{VariantAvailable, VariantAvailableProvider};

pub struct MapProviderVariantAvailable;

impl VariantAvailableProvider<MapProvider> for MapProviderVariantAvailable {
    fn is_variant_enabled(ctx: &ViewerContext<'_>, variant: MapProvider) -> VariantAvailable {
        let map_box_available = if ctx
            .app_options
            .mapbox_access_token()
            .is_some_and(|token| !token.is_empty())
        {
            VariantAvailable::Yes
        } else {
            VariantAvailable::No {
                reason_markdown: "Mapbox access token is not set. ".to_owned(),
            }
        };

        match variant {
            MapProvider::OpenStreetMap => VariantAvailable::Yes,

            MapProvider::MapboxStreets | MapProvider::MapboxDark | MapProvider::MapboxSatellite => {
                map_box_available
            }
        }
    }
}
