//! Stupid logging of many individual scalars
//!
//! Similar to `plot_dashboard_stress`.

use rand::prelude::*;
use rerun::EntityPath;
use rerun::external::re_log;

#[derive(Debug, Clone, clap::Parser)]
pub struct ScalarsCommand {
    /// Number of entities to log scalars for.
    #[arg(long = "num-entities", default_value_t = 600)]
    num_entities: usize,

    /// Number of scalars per entity.
    #[arg(long = "scalars-per-entity", default_value_t = 1)]
    scalars_per_entity: usize,

    /// Number of time steps to log.
    #[arg(long = "num-time-steps", default_value_t = 1000)]
    num_time_steps: usize,
}

type TimeSnapshot = Vec<(EntityPath, Vec<f32>)>;

struct Input {
    time_steps: Vec<TimeSnapshot>,
}

impl ScalarsCommand {
    pub fn run(self, rec: &rerun::RecordingStream) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let input = std::hint::black_box(self.prepare());
        self.execute(rec, input)
    }

    fn prepare(&self) -> Input {
        re_tracing::profile_function!();

        let mut current: TimeSnapshot = (0..self.num_entities)
            .map(|i| {
                (
                    EntityPath::from(format!("scalar_{i}")),
                    vec![0.0; self.scalars_per_entity],
                )
            })
            .collect();

        let mut time_steps = vec![current.clone()];

        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);

        for _ in 1..self.num_time_steps {
            // Random walk:
            for (_, scalars) in &mut current {
                for scalar in scalars {
                    *scalar += rng.random_range(-0.1..=0.1);
                }
            }
            time_steps.push(current.clone());
        }

        let total_log_calls = self.num_entities * self.num_time_steps;
        re_log::info!(
            "Logging {} scalars across {} time steps ({} total log calls)",
            self.num_entities,
            self.num_time_steps,
            total_log_calls
        );

        Input { time_steps }
    }

    fn execute(self, rec: &rerun::RecordingStream, input: Input) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let Input { time_steps } = input;
        let total_log_calls = self.num_entities * self.num_time_steps;

        let start = std::time::Instant::now();

        for (time_index, time_snapshot) in time_steps.into_iter().enumerate() {
            re_tracing::profile_scope!("log_time_step");

            for (entity_path, scalars) in time_snapshot {
                re_tracing::profile_scope!("log_entity");

                #[expect(clippy::cast_possible_wrap)] // usize -> i64 is fine
                rec.set_time_sequence("frame", time_index as i64);
                rec.log(entity_path.clone(), &rerun::Scalars::new(scalars))?;
            }
        }

        let elapsed = start.elapsed();
        let scalars_per_second = total_log_calls as f64 / elapsed.as_secs_f64();
        re_log::info!(
            "Logged {} scalars in {:.2?} ({:.0} scalars/second)",
            total_log_calls,
            elapsed,
            scalars_per_second
        );

        Ok(())
    }
}
