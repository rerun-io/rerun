#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, re_byte_size::SizeBytes)]
pub enum WindowFunction {
    #[default]
    Rectangular,
    Hanning,
    Hamming,
}

impl WindowFunction {
    pub const ALL: [Self; 3] = [Self::Rectangular, Self::Hanning, Self::Hamming];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangular => "rect",
            Self::Hanning => "hanning",
            Self::Hamming => "hamming",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, re_byte_size::SizeBytes)]
pub enum FilterKind {
    #[default]
    Off,
    LowPass,
    HighPass,
    BandPass,
}

impl FilterKind {
    pub const ALL: [Self; 4] = [Self::Off, Self::LowPass, Self::HighPass, Self::BandPass];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::LowPass => "low-pass",
            Self::HighPass => "high-pass",
            Self::BandPass => "band-pass",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct AudioProcessingSettings {
    pub window: WindowFunction,
    pub filter: FilterKind,
    pub low_cut_hz: f64,
    pub high_cut_hz: f64,
}

impl Default for AudioProcessingSettings {
    fn default() -> Self {
        Self {
            window: WindowFunction::Rectangular,
            filter: FilterKind::Off,
            low_cut_hz: 300.0,
            high_cut_hz: 3_000.0,
        }
    }
}

impl AudioProcessingSettings {
    pub fn is_active(&self) -> bool {
        self.window != WindowFunction::Rectangular || self.filter != FilterKind::Off
    }
}

pub fn process_samples(
    samples: &[f64],
    sample_rate: f64,
    settings: &AudioProcessingSettings,
) -> Vec<f64> {
    let mut processed = samples.to_vec();

    match settings.filter {
        FilterKind::Off => {}
        FilterKind::LowPass => {
            apply_low_pass(&mut processed, sample_rate, settings.high_cut_hz);
        }
        FilterKind::HighPass => {
            apply_high_pass(&mut processed, sample_rate, settings.low_cut_hz);
        }
        FilterKind::BandPass => {
            apply_high_pass(&mut processed, sample_rate, settings.low_cut_hz);
            apply_low_pass(&mut processed, sample_rate, settings.high_cut_hz);
        }
    }

    apply_window(&mut processed, settings.window);
    processed
}

fn apply_window(samples: &mut [f64], window: WindowFunction) {
    if window == WindowFunction::Rectangular || samples.len() <= 1 {
        return;
    }

    let len_minus_one = (samples.len() - 1) as f64;
    for (idx, sample) in samples.iter_mut().enumerate() {
        let phase = std::f64::consts::TAU * idx as f64 / len_minus_one;
        let weight = match window {
            WindowFunction::Rectangular => 1.0,
            WindowFunction::Hanning => 0.5 - 0.5 * phase.cos(),
            WindowFunction::Hamming => 0.54 - 0.46 * phase.cos(),
        };
        *sample *= weight;
    }
}

fn apply_low_pass(samples: &mut [f64], sample_rate: f64, cutoff_hz: f64) {
    let Some(alpha) = low_pass_alpha(sample_rate, cutoff_hz) else {
        return;
    };

    let mut previous = samples.first().copied().unwrap_or_default();
    for sample in samples {
        previous += alpha * (*sample - previous);
        *sample = previous;
    }
}

fn apply_high_pass(samples: &mut [f64], sample_rate: f64, cutoff_hz: f64) {
    let Some(alpha) = high_pass_alpha(sample_rate, cutoff_hz) else {
        return;
    };

    let mut previous_input = samples.first().copied().unwrap_or_default();
    let mut previous_output = 0.0;
    for sample in samples {
        let input = *sample;
        let output = alpha * (previous_output + input - previous_input);
        *sample = output;
        previous_input = input;
        previous_output = output;
    }
}

fn low_pass_alpha(sample_rate: f64, cutoff_hz: f64) -> Option<f64> {
    if !sample_rate.is_finite() || sample_rate <= 0.0 || !cutoff_hz.is_finite() || cutoff_hz <= 0.0
    {
        return None;
    }

    let dt = 1.0 / sample_rate;
    let rc = 1.0 / (std::f64::consts::TAU * cutoff_hz.min(sample_rate * 0.49));
    Some(dt / (rc + dt))
}

fn high_pass_alpha(sample_rate: f64, cutoff_hz: f64) -> Option<f64> {
    if !sample_rate.is_finite() || sample_rate <= 0.0 || !cutoff_hz.is_finite() || cutoff_hz <= 0.0
    {
        return None;
    }

    let dt = 1.0 / sample_rate;
    let rc = 1.0 / (std::f64::consts::TAU * cutoff_hz.min(sample_rate * 0.49));
    Some(rc / (rc + dt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hanning_window_tapers_edges() {
        let settings = AudioProcessingSettings {
            window: WindowFunction::Hanning,
            ..Default::default()
        };

        let processed = process_samples(&[1.0, 1.0, 1.0, 1.0, 1.0], 16_000.0, &settings);

        assert!(processed[0].abs() < 1e-9);
        assert!(processed[2] > 0.99);
        assert!(processed[4].abs() < 1e-9);
    }

    #[test]
    fn high_pass_filter_suppresses_dc_signal() {
        let settings = AudioProcessingSettings {
            filter: FilterKind::HighPass,
            low_cut_hz: 100.0,
            ..Default::default()
        };

        let processed = process_samples(&vec![1.0; 512], 16_000.0, &settings);

        assert!(processed.last().copied().unwrap_or_default().abs() < 0.05);
    }
}
