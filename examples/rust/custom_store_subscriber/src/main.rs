//! This example demonstrates how to use [`StoreSubscriber`]s and [`StoreEvent`]s to implement both
//! custom secondary indices and trigger systems.
//!
//! Usage:
//! ```sh
//! # Start the Rerun Viewer with our custom view in a terminal:
//! $ cargo r -p custom_store_subscriber
//!
//! # Log any kind of data from another terminal:
//! $ cargo r -p objectron -- --connect
//! ```

use std::collections::BTreeMap;

use rerun::{
    external::{anyhow, re_build_info, re_data_store, re_log, re_log_types::TimeRange, tokio},
    time::TimeInt,
    ComponentName, EntityPath, StoreEvent, StoreId, StoreSubscriber, Timeline,
};

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    re_log::setup_logging();

    let _handle = re_data_store::DataStore::register_subscriber(Box::<Orchestrator>::default());
    // Could use the returned handle to get a reference to the view if needed.

    let build_info = re_build_info::build_info!();
    rerun::run(build_info, rerun::CallSource::Cli, std::env::args())
        .await
        .map(std::process::ExitCode::from)
}

// ---

/// A meta [`StoreSubscriber`] that distributes work to our other views.
///
/// The order is which registered views are executed is undefined: if you rely on a specific order
/// of execution between your views, orchestrate it yourself!
///
/// Clears the terminal and resets the cursor for every new batch of [`StoreEvent`]s.
#[derive(Default)]
struct Orchestrator {
    components_per_recording: ComponentsPerRecording,
    time_ranges_per_entity: TimeRangesPerEntity,
}

impl StoreSubscriber for Orchestrator {
    fn name(&self) -> String {
        "rerun.store_subscriber.ScreenClearer".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        print!("\x1B[2J\x1B[1;1H"); // terminal clear + cursor reset

        self.components_per_recording.on_events(events);
        self.time_ranges_per_entity.on_events(events);
    }
}

// ---

/// A [`StoreSubscriber`] that maintains a secondary index that keeps count of the number of occurrences
/// of each component in each [`rerun::DataStore`].
///
/// It also implements a trigger that prints to the console each time a component is first introduced
/// and retired.
///
/// For every [`StoreEvent`], it displays the state of the secondary index to the terminal.
#[derive(Default, Debug, PartialEq, Eq)]
struct ComponentsPerRecording {
    counters: BTreeMap<StoreId, BTreeMap<ComponentName, u64>>,
}

impl StoreSubscriber for ComponentsPerRecording {
    fn name(&self) -> String {
        "rerun.store_subscriber.ComponentsPerRecording".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        for event in events {
            // update counters
            let per_component = self.counters.entry(event.store_id.clone()).or_default();
            for &component_name in event.cells.keys() {
                let count = per_component.entry(component_name).or_default();

                // if first occurrence, speak!
                if event.delta() > 0 && *count == 0 {
                    println!(
                        "New component introduced in recording {}: {}!",
                        event.store_id, component_name,
                    );
                }
                // if last occurrence, speak!
                else if event.delta() < 0 && *count <= event.delta().unsigned_abs() {
                    println!(
                        "Component retired from recording {}: {}!",
                        event.store_id, component_name,
                    );
                }

                *count = count.saturating_add_signed(event.delta());
            }
        }

        if self.counters.is_empty() {
            return;
        }

        println!("Component stats");
        println!("---------------");

        for (recording, per_component) in &self.counters {
            println!("  Recording '{recording}':");
            for (component, counter) in per_component {
                println!("    {component}: {counter} occurrences");
            }
        }
    }
}

// ---

/// A [`StoreSubscriber`] that maintains a secondary index of the time ranges covered by each entity,
/// on every timeline, across all recordings (i.e. [`rerun::DataStore`]s).
///
/// For every [`StoreEvent`], it displays the state of the secondary index to the terminal.
#[derive(Default, Debug, PartialEq, Eq)]
struct TimeRangesPerEntity {
    times: BTreeMap<EntityPath, BTreeMap<Timeline, BTreeMap<TimeInt, u64>>>,
}

impl StoreSubscriber for TimeRangesPerEntity {
    fn name(&self) -> String {
        "rerun.store_subscriber.TimeRangesPerEntity".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        for event in events {
            for &(timeline, time) in &event.times {
                // update counters
                let per_timeline = self.times.entry(event.entity_path.clone()).or_default();
                let per_time = per_timeline.entry(timeline).or_default();
                let count = per_time.entry(time).or_default();

                *count = count.saturating_add_signed(event.delta());

                if *count == 0 {
                    per_time.remove(&time);
                }
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
                    times
                        .first_key_value()
                        .map_or(TimeInt::MIN, |(time, _)| *time),
                    times
                        .last_key_value()
                        .map_or(TimeInt::MAX, |(time, _)| *time),
                );
                let time_range = timeline.format_time_range_utc(&time_range);
                println!("  {time_range}");
            }
        }
    }
}
