/// Formats the value of an axis tick with as many decimals as the step to the next tick needs.
///
/// For example, a step of `0.01` gives two decimals.
pub fn format_axis_tick(value: f64, step_size: f64) -> String {
    let num_decimals = -step_size.log10().round() as usize;

    re_format::FloatFormatOptions::DEFAULT_f64
        .with_decimals(num_decimals)
        .format(value)
}

/// Formats the value of an axis tick like [`format_axis_tick`], with at most `max_chars`
/// characters where possible.
///
/// Longer values are shown in scientific notation.
pub fn format_axis_tick_with_max_chars(value: f64, step_size: f64, max_chars: usize) -> String {
    let text = format_axis_tick(value, step_size);
    if text.chars().count() <= max_chars {
        return text;
    }

    let sign = if value < 0.0 {
        re_format::MINUS.to_string()
    } else {
        String::new()
    };
    let value = value.abs();

    // As many decimals as the step size needs to tell neighboring ticks apart, as long as the
    // label stays within `max_chars`. The decimals also need room for the `.`.
    let decimals = (value.log10().floor() - step_size.log10().round()).max(0.0) as usize;
    let without_decimals = sign.chars().count() + format!("{value:.0e}").len();
    let decimals = decimals.min(max_chars.saturating_sub(without_decimals + 1));
    format!("{sign}{value:.decimals$e}")
}
