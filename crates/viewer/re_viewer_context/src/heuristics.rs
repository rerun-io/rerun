use re_chunk::Timeline;
use re_log_channel::LogSource;
use re_log_types::EntityPath;

use crate::{
    IdentifiedViewSystem, RecommendedView, ViewSpawnHeuristics, ViewerContext, VisualizerSystem,
};

/// Spawns a view for each single entity which is visualizable & indicator-matching for a given visualizer.
///
/// This is used as utility by *some* view types that want
/// to spawn a view for every single entity that is visualizable with a given visualizer.
pub fn suggest_view_for_each_entity<TVisualizer>(
    ctx: &ViewerContext<'_>,
    include_entity: &dyn Fn(&EntityPath) -> bool,
) -> ViewSpawnHeuristics
where
    TVisualizer: VisualizerSystem + IdentifiedViewSystem + Default,
{
    re_tracing::profile_function!();

    let Some(indicator_matching_entities) = ctx
        .indicated_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return ViewSpawnHeuristics::empty();
    };
    let Some(visualizable_entities) = ctx
        .visualizable_entities_per_visualizer
        .get(&TVisualizer::identifier())
    else {
        return ViewSpawnHeuristics::empty();
    };

    let recommended_views = indicator_matching_entities
        .iter()
        .filter(|entity| visualizable_entities.contains_key(entity))
        .filter_map(|entity| {
            if include_entity(entity) {
                Some(RecommendedView::new_single_entity(entity.clone()))
            } else {
                None
            }
        });

    ViewSpawnHeuristics::new(recommended_views)
}

/// Heuristic to pick a preferred timeline for a given type of data source, if any.
pub fn preferred_timeline_for_log_source(
    data_source: Option<&LogSource>,
    available_timelines: &[&Timeline],
) -> Option<Timeline> {
    // The MCAP loader creates multiple timelines, and we want to have a consistent default selected.
    // `message_log_time` is a good preference here - it's present in any MCAP, and also used by other tools as default.
    if is_mcap_source(Some(data_source?)) {
        return available_timelines.iter().find_map(|timeline| {
            (timeline.name().as_str() == "message_log_time").then_some(**timeline)
        });
    }

    None
}

fn is_mcap_source(log_source: Option<&LogSource>) -> bool {
    fn url_contains_mcap(path: &str) -> bool {
        path.split(['?', '#'])
            .next()
            .is_some_and(|path| path.to_ascii_lowercase().ends_with(".mcap"))
    }

    match log_source {
        Some(LogSource::File { path, .. }) => path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mcap")),
        Some(LogSource::HttpStream { url, .. }) => url_contains_mcap(url),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use re_log_channel::LogSource;
    use re_log_types::TimeType;

    use super::*;

    #[test]
    fn test_is_mcap_source() {
        assert!(is_mcap_source(Some(&LogSource::File {
            path: PathBuf::from("/tmp/data.mcap"),
            follow: false,
        })));
        assert!(is_mcap_source(Some(&LogSource::File {
            path: PathBuf::from("/tmp/data.MCAP"),
            follow: false,
        })));
        assert!(is_mcap_source(Some(&LogSource::HttpStream {
            url: "https://example.com/data.mcap".to_owned(),
            follow: false,
        })));
        assert!(is_mcap_source(Some(&LogSource::HttpStream {
            url: "https://example.com/data.mcap?download=1#view".to_owned(),
            follow: false,
        })));

        assert!(!is_mcap_source(Some(&LogSource::File {
            path: PathBuf::from("/tmp/data.rrd"),
            follow: false,
        })));
        assert!(!is_mcap_source(Some(&LogSource::Sdk)));
        assert!(!is_mcap_source(None));
    }

    #[test]
    fn test_mcap_default_timeline_prefers_message_log_time() {
        let message_log_time = Timeline::new("message_log_time", TimeType::TimestampNs);
        let log_time = Timeline::log_time();
        let timelines = [&message_log_time, &log_time];
        let data_source = LogSource::File {
            path: PathBuf::from("/tmp/data.mcap"),
            follow: false,
        };
        assert_eq!(
            preferred_timeline_for_log_source(Some(&data_source), &timelines),
            Some(message_log_time)
        );
    }
}
