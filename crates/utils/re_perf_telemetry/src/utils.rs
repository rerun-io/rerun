//! Utility functions for various telemetry-related tasks.

/// Displays a possibly long list of items to a length limited list of items.
///
/// ### Examples
/// ```
/// use re_perf_telemetry::to_short_str;
///
/// let long_list = [1, 2, 3, 4, 5];
/// let max_len = 3;
/// let result = to_short_str(long_list, max_len);
/// assert_eq!(result, "[1,2,3,..]");
///
/// let long_list = [1, 2];
/// let max_len = 3;
/// let result = to_short_str(long_list, max_len);
/// assert_eq!(result, "[1,2]");
///
/// let long_list: [u8; 0] = [];
/// let max_len = 3;
/// let result = to_short_str(long_list, max_len);
/// assert_eq!(result, "[]");
///
/// let long_list = [1, 2, 3, 4, 5];
/// let max_len = 0;
/// let result = to_short_str(long_list, max_len);
/// assert_eq!(result, "[..]");
/// ```
pub fn to_short_str<T: IntoIterator<Item = K>, K: std::fmt::Display>(
    long_list: T,
    max_len: usize,
) -> String {
    let mut short_list = String::new();
    short_list.push('[');

    let iter = long_list.into_iter();

    for (count, item) in iter.enumerate() {
        if count > 0 {
            short_list.push(',');
        }
        if count >= max_len {
            short_list.push_str("..");
            break;
        }
        short_list.push_str(&item.to_string());
    }
    short_list.push(']');

    short_list
}
