//! Benchmark for logging `Transform3D`

use rerun::external::re_log;

use crate::lcg;

#[derive(Debug, Clone, clap::Parser)]
pub struct Transform3DCommand {
    /// Log all transforms as static?
    #[arg(long = "static")]
    static_: bool,

    /// Number of entities to log transforms for.
    #[arg(long = "num-entities", default_value_t = 10)]
    num_entities: usize,

    /// Number of time steps to log.
    #[arg(long = "num-time-steps", default_value_t = 10_000)]
    num_time_steps: usize,
}

struct Transform {
    translation: [f32; 3],
    mat3x3: [f32; 9],
}

struct Input {
    /// For each time step, a list of transforms per entity.
    time_steps: Vec<Vec<Transform>>,
}

impl Transform3DCommand {
    pub fn run(self, rec: &rerun::RecordingStream) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let input = std::hint::black_box(self.prepare());
        self.execute(rec, input)
    }

    fn prepare(&self) -> Input {
        re_tracing::profile_function!();

        let mut lcg_state: i64 = 12345;

        let mut time_steps = Vec::with_capacity(self.num_time_steps);

        for _ in 0..self.num_time_steps {
            let mut transforms = Vec::with_capacity(self.num_entities);
            for _ in 0..self.num_entities {
                // Generate pseudo-random but deterministic transform data
                let translation = [
                    (lcg(&mut lcg_state) % 1000) as f32 / 100.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 100.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 100.0,
                ];

                // Generate a mat3x3 (9 floats)
                let mat3x3 = [
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                    (lcg(&mut lcg_state) % 1000) as f32 / 500.0 - 1.0,
                ];

                transforms.push(Transform {
                    translation,
                    mat3x3,
                });
            }
            time_steps.push(transforms);
        }

        re_log::info!(
            "Logging {} transforms across {} time steps ({} total log calls)",
            self.num_entities,
            self.num_time_steps,
            self.num_entities * self.num_time_steps
        );

        Input { time_steps }
    }

    fn execute(self, rec: &rerun::RecordingStream, input: Input) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let Input { time_steps } = input;
        let total_log_calls = self.num_entities * self.num_time_steps;

        let start = std::time::Instant::now();

        for (time_index, transforms) in time_steps.into_iter().enumerate() {
            re_tracing::profile_scope!("log_time_step");

            for (entity_index, transform) in transforms.into_iter().enumerate() {
                re_tracing::profile_scope!("log_entity");

                let entity_path = format!("transform_{entity_index}");

                let transform3d = rerun::Transform3D::default()
                    .with_translation(transform.translation)
                    .with_mat3x3(transform.mat3x3);

                if self.static_ {
                    rec.log_with_static(entity_path, true, &transform3d)?;
                } else {
                    #[expect(clippy::cast_possible_wrap)]
                    rec.set_time_sequence("frame", time_index as i64);
                    rec.log(entity_path, &transform3d)?;
                }
            }
        }

        let elapsed = start.elapsed();
        let transforms_per_second = total_log_calls as f64 / elapsed.as_secs_f64();
        re_log::info!(
            "Logged {} transforms in {:.2?} ({:.0} transforms/second)",
            total_log_calls,
            elapsed,
            transforms_per_second
        );

        Ok(())
    }
}
