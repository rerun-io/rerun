//! This example demonstrates how to use [`ChunkStoreSubscriber`]s and [`ChunkStoreEvent`]s to implement both
//! custom secondary indices and trigger systems.
//!
//! Usage:
//! ```sh
//! # Start the Rerun Viewer with our custom view in a terminal:
//! $ cargo r -p custom_store_subscriber
//!
//! # Log any kind of data from another terminal:
//! $ cargo r -p objectron -- --connect
//! ````

use std::collections::BTreeMap;

use rerun::{
    external::{anyhow, re_build_info, re_chunk_store, re_log, re_log_types::ResolvedTimeRange},
    time::TimeInt,
    ChunkStoreEvent, ChunkStoreSubscriber, ComponentName, EntityPath, StoreId, Timeline,
};

fn main() -> anyhow::Result<std::process::ExitCode> {
    let main_thread_token = rerun::MainThreadToken::i_promise_i_am_on_the_main_thread();
    re_log::setup_logging();

    let _handle = re_chunk_store::ChunkStore::register_subscriber(Box::<Orchestrator>::default());
    // Could use the returned handle to get a reference to the view if needed.

    let build_info = re_build_info::build_info!();
    rerun::run(
        main_thread_token,
        build_info,
        rerun::CallSource::Cli,
        std::env::args(),
    )
    .map(std::process::ExitCode::from)
}

// ---

/// A meta [`ChunkStoreSubscriber`] that distributes work to our other views.
///
/// The order is which registered views are executed is undefined: if you rely on a specific order
/// of execution between your views, orchestrate it yourself!
///
/// Clears the terminal and resets the cursor for every new batch of [`ChunkStoreEvent`]s.
#[derive(Default)]
struct Orchestrator {
    components_per_recording: ComponentsPerRecording,
    time_ranges_per_entity: TimeRangesPerEntity,
}

impl ChunkStoreSubscriber for Orchestrator {
    fn name(&self) -> String {
        "rerun.store_subscriber.ScreenClearer".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        print!("\x1B[2J\x1B[1;1H"); // terminal clear + cursor reset

        self.components_per_recording.on_events(events);
        self.time_ranges_per_entity.on_events(events);
    }
}

// ---

/// A [`ChunkStoreSubscriber`] that maintains a secondary index that keeps count of the number of occurrences
/// of each component in each [`rerun::ChunkStore`].
///
/// It also implements a trigger that prints to the console each time a component is first introduced
/// and retired.
///
/// For every [`ChunkStoreEvent`], it displays the state of the secondary index to the terminal.
#[derive(Default, Debug, PartialEq, Eq)]
struct ComponentsPerRecording {
    counters: BTreeMap<StoreId, BTreeMap<ComponentName, u64>>,
}

impl ChunkStoreSubscriber for ComponentsPerRecording {
    fn name(&self) -> String {
        "rerun.store_subscriber.ComponentsPerRecording".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        for event in events {
            // update counters
            let per_component = self.counters.entry(event.store_id.clone()).or_default();
            for component_name in event.chunk.component_names() {
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
            println!("  Recording '{recording}':"); // NOLINT
            for (component, counter) in per_component {
                println!("    {component}: {counter} occurrences");
            }
        }
    }
}

// ---

/// A [`ChunkStoreSubscriber`] that maintains a secondary index of the time ranges covered by each entity,
/// on every timeline, across all recordings (i.e. [`rerun::ChunkStore`]s).
///
/// For every [`ChunkStoreEvent`], it displays the state of the secondary index to the terminal.
#[derive(Default, Debug, PartialEq, Eq)]
struct TimeRangesPerEntity {
    times: BTreeMap<EntityPath, BTreeMap<Timeline, BTreeMap<TimeInt, u64>>>,
}

impl ChunkStoreSubscriber for TimeRangesPerEntity {
    fn name(&self) -> String {
        "rerun.store_subscriber.TimeRangesPerEntity".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        for event in events {
            for (timeline, time_column) in event.chunk.timelines() {
                for time in time_column.times() {
                    // update counters
                    let per_timeline = self
                        .times
                        .entry(event.chunk.entity_path().clone())
                        .or_default();
                    let per_time = per_timeline.entry(*timeline).or_default();
                    let count = per_time.entry(time).or_default();

                    *count = count.saturating_add_signed(event.delta());

                    if *count == 0 {
                        per_time.remove(&time);
                    }
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
                let time_range = ResolvedTimeRange::new(
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
