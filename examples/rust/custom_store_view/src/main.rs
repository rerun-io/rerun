// TODO: doc

use std::collections::{BTreeMap, BTreeSet};

use re_arrow_store::{StoreEvent, StoreView};
use rerun::{
    external::{anyhow, re_arrow_store, re_build_info, re_log, re_log_types::TimeRange, tokio},
    time::TimeInt,
    ComponentName, EntityPath, StoreId, Timeline,
};

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    re_log::setup_native_logging();

    let _handle = re_arrow_store::DataStore::register_view(Box::<ScreenClearer>::default());
    let _handle =
        re_arrow_store::DataStore::register_view(Box::<ComponentsPerRecording>::default());
    let _handle = re_arrow_store::DataStore::register_view(Box::<TimeRangesPerEntity>::default());

    let build_info = re_build_info::build_info!();
    rerun::run(build_info, rerun::CallSource::Cli, std::env::args())
        .await
        .map(std::process::ExitCode::from)
}

// ---

/// A [`StoreView`] that simply clears the terminal and resets the cursor for every new batch of [`StoreEvent`]s.
#[derive(Default)]
struct ScreenClearer;

impl StoreView for ScreenClearer {
    fn name(&self) -> String {
        "rerun.store_view.ScreenClearer".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, _events: &[StoreEvent]) {
        print!("\x1B[2J\x1B[1;1H"); // terminal clear + cursor reset
    }
}

// ---

#[derive(Default, Debug, PartialEq, Eq)]
struct ComponentsPerRecording {
    counters: BTreeMap<StoreId, BTreeMap<ComponentName, u64>>,
}

impl StoreView for ComponentsPerRecording {
    fn name(&self) -> String {
        "rerun.store_view.ComponentsPerRecording".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        for event in events {
            let diff = &event.diff;

            // update counters
            let per_component = self.counters.entry(event.store_id.clone()).or_default();
            let count = per_component.entry(diff.component_name).or_default();

            // if first occurence, speak!
            if diff.delta > 0 && *count == 0 {
                println!(
                    "New component introduced in recording {}: {}!",
                    event.store_id, diff.component_name,
                );
            }
            // if last occurence, speak!
            else if diff.delta < 0 && *count <= diff.delta.unsigned_abs() {
                println!(
                    "Component removed from recording {}: {}!",
                    event.store_id, diff.component_name,
                );
            }

            *count = count.saturating_add_signed(diff.delta);
        }

        if self.counters.is_empty() {
            return;
        }

        println!("Component stats");
        println!("---------------");

        for (recording, per_component) in &self.counters {
            println!("  Recording '{recording}':");
            for (component, counter) in per_component {
                println!("    {component}: {counter} occurences");
            }
        }
    }
}

// ---

#[derive(Default, Debug, PartialEq, Eq)]
struct TimeRangesPerEntity {
    times: BTreeMap<EntityPath, BTreeMap<Timeline, BTreeSet<TimeInt>>>,
    counters: BTreeMap<EntityPath, BTreeMap<Timeline, BTreeMap<TimeInt, u64>>>,
}

impl StoreView for TimeRangesPerEntity {
    fn name(&self) -> String {
        "rerun.store_view.TimeRangesPerEntity".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        for event in events {
            let diff = &event.diff;

            if let Some((timeline, time)) = diff.timestamp {
                // update counters
                let per_timeline = self.counters.entry(diff.entity_path.clone()).or_default();
                let per_time = per_timeline.entry(timeline).or_default();
                let count = per_time.entry(time).or_default();

                // if first occurence, update time ranges
                if diff.delta > 0 && *count == 0 {
                    let per_timeline = self.times.entry(diff.entity_path.clone()).or_default();
                    let per_time = per_timeline.entry(timeline).or_default();
                    per_time.insert(time);
                }
                // if last occurence, update time ranges
                else if diff.delta < 0 && *count <= diff.delta.unsigned_abs() {
                    let per_timeline = self.times.entry(diff.entity_path.clone()).or_default();
                    let per_time = per_timeline.entry(timeline).or_default();
                    per_time.remove(&time);
                }

                *count = count.saturating_add_signed(diff.delta);
            }
        }

        if self.times.is_empty() {
            return;
        }

        println!("Entity time ranges");
        println!("------------------");

        for (entity_path, per_timeline) in &self.times {
            println!("  {entity_path}:");
            for (timeline, times) in per_timeline {
                let time_range = TimeRange::new(
                    times.first().copied().unwrap_or(TimeInt::MIN),
                    times.last().copied().unwrap_or(TimeInt::MAX),
                );
                let time_range = timeline.format_time_range_utc(&time_range);
                println!("  {time_range}");
            }
        }
    }
}
