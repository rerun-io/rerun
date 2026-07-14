use itertools::Itertools as _;
use num_traits::AsPrimitive;

/// Convert a numeric [`egui::util::History`] into a plot line.
pub fn history_to_plot<'a, T>(name: &str, history: &egui::util::History<T>) -> egui_plot::Line<'a>
where
    T: Copy + AsPrimitive<f64>,
{
    egui_plot::Line::new(
        name,
        history
            .iter()
            .map(|(time, val)| [time, val.as_()])
            .collect_vec(),
    )
}
