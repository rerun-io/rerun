//! Plot dashboard stress test.
//!
//! Similar to `tests/rust/log_benchmark/src/scalars.rs`.
//!
//! Usage:
//! ```text
//! pixi run rs-plot-dashboard --help
//! ```
//!
//! Example:
//! ```text
//! pixi run rs-plot-dashboard --num-plots 10 --num-series-per-plot 5 --num-points-per-series 5000 --freq 1000
//! ```

#![expect(clippy::cast_possible_wrap)]
#![expect(clippy::disallowed_methods)]

use rerun::external::re_log;

#[derive(Debug, clap::ValueEnum, Clone)]
enum Order {
    Forwards,
    Backwards,
    Random,
}

#[derive(Debug, clap::ValueEnum, Clone)]
enum SeriesType {
    SinUniform,
    GaussianRandomWalk,
}

// TODO(cmc): could have flags to add attributes (color, radius...) to put some more stress
// on the line fragmenter.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// How many different plots?
    #[clap(long, default_value = "1")]
    num_plots: u64,

    /// How many series in each single plot?
    #[clap(long, default_value = "1")]
    num_series_per_plot: u64,

    /// How many points in each single series?
    #[clap(long, default_value = "10000")]
    num_points_per_series: u64,

    /// Frequency of logging (applies to all series).
    #[clap(long, default_value = "1000.0")]
    freq: f64,

    /// Frequency of logging (applies to all series).
    #[clap(long, default_value = None)]
    temporal_batch_size: Option<u64>,

    /// What order to log the data in (applies to all series).
    #[clap(long, value_enum, default_value = "forwards")]
    order: Order,

    /// The method used to generate time series.
    #[clap(long, value_enum, default_value = "gaussian-random-walk")]
    series_type: SeriesType,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_plot_dashboard_stress")?;
    run(&rec, &args)
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    let plot_paths: Vec<_> = (0..args.num_plots).map(|i| format!("plot_{i}")).collect();
    let series_paths: Vec<_> = (0..args.num_series_per_plot)
        .map(|i| format!("series_{i}"))
        .collect();

    let num_series = args.num_plots * args.num_series_per_plot;
    let mut time_per_tick = 1.0 / args.freq;
    let mut scalars_per_tick = num_series;
    if let Some(temporal_batch_size) = args.temporal_batch_size {
        time_per_tick *= temporal_batch_size as f64;
        scalars_per_tick *= temporal_batch_size;
    }

    let expected_total_freq = args.freq * num_series as f64;

    use rand::Rng as _;
    let mut rng = rand::rng();
    let distr_uniform_pi = rand::distr::Uniform::new(0f64, std::f64::consts::PI)?;
    let distr_std_normal = rand_distr::StandardNormal;

    let mut sim_times: Vec<f64> = (0..args.num_points_per_series as i64)
        .map(|i| time_per_tick * i as f64)
        .collect();
    match args.order {
        Order::Forwards => {}
        Order::Backwards => sim_times.reverse(),
        Order::Random => {
            use rand::seq::SliceRandom as _;
            sim_times.shuffle(&mut rng);
        }
    }

    let values_per_series: Vec<Vec<f64>> = std::iter::from_fn(|| {
        let mut value = 0.0;
        let values = (0..args.num_points_per_series)
            .map(|_| {
                match args.series_type {
                    SeriesType::SinUniform => value = rng.sample(distr_uniform_pi).sin(),
                    SeriesType::GaussianRandomWalk => {
                        value += rng.sample::<f64, _>(distr_std_normal);
                    }
                }
                value
            })
            .collect();
        Some(values)
    })
    .take(num_series as _)
    .collect();

    let offsets = (0..sim_times.len())
        .step_by(args.temporal_batch_size.unwrap_or(1) as usize)
        .collect::<Vec<_>>();

    let mut total_num_scalars = 0;
    let mut total_start_time = std::time::Instant::now();
    let mut max_load = 0.0;

    let mut tick_start_time = std::time::Instant::now();

    #[expect(clippy::unchecked_time_subtraction)]
    for offset in offsets {
        if args.temporal_batch_size.is_none() {
            rec.set_duration_secs("sim_time", sim_times[offset]);
        }

        // Log

        for (plot_idx, plot_path) in plot_paths.iter().enumerate() {
            let plot_idx = plot_idx * args.num_series_per_plot as usize;
            for (series_idx, series_path) in series_paths.iter().enumerate() {
                let path = format!("{plot_path}/{series_path}");
                let series_values = &values_per_series[plot_idx + series_idx];
                if let Some(temporal_batch_size) = args.temporal_batch_size {
                    let temporal_batch_size = temporal_batch_size as usize;
                    let seconds = sim_times.iter().skip(offset).take(temporal_batch_size);
                    let values = series_values.iter().skip(offset).take(temporal_batch_size);
                    rec.send_columns(
                        path,
                        [rerun::TimeColumn::new_duration_secs(
                            "sim_time",
                            seconds.copied(),
                        )],
                        rerun::Scalars::new(values.copied()).columns_of_unit_batches()?,
                    )?;
                } else {
                    rec.log(path, &rerun::Scalars::single(series_values[offset]))?;
                }
            }
        }

        // Measure how long this took and how high the load was.

        let elapsed = tick_start_time.elapsed();
        max_load = f64::max(max_load, elapsed.as_secs_f64() / time_per_tick);

        // Throttle

        let sleep_duration = time_per_tick - elapsed.as_secs_f64();
        if sleep_duration > 0.0 {
            let sleep_duration = std::time::Duration::from_secs_f64(sleep_duration);
            let sleep_start_time = std::time::Instant::now();
            std::thread::sleep(sleep_duration);

            // We will very likely be put to sleep for more than we asked for, and therefore need
            // to pay off that debt in order to meet our frequency goal.
            let sleep_debt = sleep_start_time.elapsed() - sleep_duration;
            tick_start_time = std::time::Instant::now() - sleep_debt;
        } else {
            tick_start_time = std::time::Instant::now();
        }

        // Progress report
        //
        // Must come after throttle since we report every wall-clock second:
        // If ticks are large & fast, then after each send we run into throttle.
        // So if this was before throttle, we'd not report the first tick no matter how large it was.

        total_num_scalars += scalars_per_tick;
        let total_elapsed = total_start_time.elapsed();
        if total_elapsed.as_secs_f64() >= 1.0 {
            println!(
                "logged {total_num_scalars} scalars over {:?} (freq={:.3}Hz, expected={expected_total_freq:.3}Hz, load={:.3}%)",
                total_elapsed,
                total_num_scalars as f64 / total_elapsed.as_secs_f64(),
                max_load * 100.0,
            );

            let elapsed_debt =
                std::time::Duration::from_secs_f64(total_elapsed.as_secs_f64().fract());
            total_start_time = std::time::Instant::now() - elapsed_debt;
            total_num_scalars = 0;
            max_load = 0.0;
        }
    }

    if total_num_scalars > 0 {
        let total_elapsed = total_start_time.elapsed();
        println!(
            "logged {total_num_scalars} scalars over {:?} (freq={:.3}Hz, expected={expected_total_freq:.3}Hz, load={:.3}%)",
            total_elapsed,
            total_num_scalars as f64 / total_elapsed.as_secs_f64(),
            max_load * 100.0,
        );
    }

    Ok(())
}
