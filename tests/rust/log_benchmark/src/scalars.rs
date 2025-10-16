//! Stupid logging of many individual scalars
//!
//! Similar to `plot_dashboard_stress`

use rand::prelude::*;
use rerun::EntityPath;

const NUM_ENTITIES: usize = 600;
const SCALARS_PER_ENTITY: usize = 1;
const NUM_TIME_STEPS: usize = 1000;

type TimeSnapshot = Vec<(EntityPath, Vec<f32>)>;
type Input = Vec<TimeSnapshot>;

fn prepare() -> Input {
    let mut current: TimeSnapshot = (0..NUM_ENTITIES)
        .map(|i| {
            (
                EntityPath::from(format!("scalar_{i}")),
                vec![0.0; SCALARS_PER_ENTITY],
            )
        })
        .collect();

    let mut time_steps = vec![current.clone()];

    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);

    for _ in 1..NUM_TIME_STEPS {
        // Random walk:
        for (_, scalars) in &mut current {
            for scalar in scalars {
                *scalar += rng.random_range(-0.1..=0.1);
            }
        }
        time_steps.push(current.clone());
    }

    time_steps
}

fn execute(rec: &rerun::RecordingStream, input: Input) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    for (time_index, time_snapshot) in input.into_iter().enumerate() {
        re_tracing::profile_scope!("log_time_step");

        for (entity_path, scalars) in time_snapshot {
            re_tracing::profile_scope!("log_entity");

            #[expect(clippy::cast_possible_wrap)] // usize -> i64 is fine
            rec.set_time_sequence("frame", time_index as i64);
            rec.log(entity_path.clone(), &rerun::Scalars::new(scalars))?;
        }
    }
    Ok(())
}

pub fn run(rec: &rerun::RecordingStream) -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare());
    execute(rec, input)
}
