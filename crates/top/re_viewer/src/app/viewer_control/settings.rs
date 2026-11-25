use re_log_channel::ViewerControlError;
use re_protos::viewer_control::v1alpha1::{
    GetSettingsResponse, SetSettingsRequest, SetSettingsResponse,
};
use re_viewer_context::AppOptions;
use serde_json::{Map, Value};

/// Settings that `get_settings` never reports and `set_settings` never changes.
const SECRET_SETTINGS: &[&str] = &["mapbox_access_token"];

/// Settings that `get_settings` reports but `set_settings` refuses to change.
///
/// The viewer runs the `FFmpeg` binary at `video.ffmpeg_path`, and writes to `cache_directory`.
const READ_ONLY_SETTINGS: &[&str] = &[
    "cache_directory",
    "video.ffmpeg_path",
    "video.override_ffmpeg_path",
];

/// `set_settings` refuses to lower the memory limit below this.
const MIN_MEMORY_LIMIT_BYTES: u64 = 128 * 1024 * 1024;

pub fn get_settings(app_options: &AppOptions) -> Result<GetSettingsResponse, ViewerControlError> {
    Ok(GetSettingsResponse {
        settings_json: to_json(app_options)?,
    })
}

/// Applies `request` to `app_options`, or changes nothing if any part of it is invalid.
pub fn set_settings(
    app_options: &mut AppOptions,
    request: SetSettingsRequest,
) -> Result<SetSettingsResponse, ViewerControlError> {
    let SetSettingsRequest { settings_json } = request;

    let patch: Value = serde_json::from_str(&settings_json).map_err(|err| {
        ViewerControlError::invalid_argument(format!("settings_json is not valid JSON: {err}"))
    })?;
    let Value::Object(patch_object) = &patch else {
        return Err(ViewerControlError::invalid_argument(
            "settings_json must be a JSON object",
        ));
    };
    if let Some(secret) = SECRET_SETTINGS
        .iter()
        .find(|key| patch_object.contains_key(**key))
    {
        return Err(ViewerControlError::invalid_argument(format!(
            "{secret:?} can only be changed in the settings screen"
        )));
    }

    let old_settings = Value::Object(to_object(app_options)?);
    let mut settings = old_settings.clone();
    merge(&mut settings, patch.clone());
    let new_options: AppOptions = serde_path_to_error::deserialize(settings).map_err(|err| {
        ViewerControlError::invalid_argument(format!(
            "Invalid value for setting {:?}: {}",
            err.path().to_string(),
            err.inner()
        ))
    })?;

    // Serde ignores unknown keys, and turns out-of-range floats into `null`,
    // so check that the new settings hold exactly what was asked for.
    let new_settings = Value::Object(to_object(&new_options)?);
    check_applied(&new_settings, &patch, "")?;

    for path in READ_ONLY_SETTINGS {
        let pointer = format!("/{}", path.replace('.', "/"));
        if old_settings.pointer(&pointer) != new_settings.pointer(&pointer) {
            return Err(ViewerControlError::invalid_argument(format!(
                "{path:?} cannot be changed with set_settings"
            )));
        }
    }

    let old_limit = app_options.memory_limit;
    let new_limit = new_options.memory_limit;
    let lowers_limit = !old_limit.is_limited() || new_limit.as_bytes() < old_limit.as_bytes();
    if new_limit.is_limited() && new_limit.as_bytes() < MIN_MEMORY_LIMIT_BYTES && lowers_limit {
        return Err(ViewerControlError::invalid_argument(format!(
            "memory_limit cannot be lowered below {}, but may be unlimited",
            re_format::format_bytes(MIN_MEMORY_LIMIT_BYTES as f64)
        )));
    }

    if app_options.experimental.gamepad_navigation && !new_options.experimental.gamepad_navigation {
        re_gamepad::clear_event_waker();
    }

    *app_options = new_options;

    Ok(SetSettingsResponse {
        settings_json: to_json(app_options)?,
    })
}

fn to_object(app_options: &AppOptions) -> Result<Map<String, Value>, ViewerControlError> {
    match serde_json::to_value(app_options) {
        Ok(Value::Object(settings)) => Ok(settings),
        Ok(other) => Err(ViewerControlError::internal(format!(
            "Settings serialized to {other}, expected a JSON object"
        ))),
        Err(err) => Err(ViewerControlError::internal(format!(
            "Failed to serialize settings: {err}"
        ))),
    }
}

fn to_json(app_options: &AppOptions) -> Result<String, ViewerControlError> {
    let mut settings = to_object(app_options)?;
    for secret in SECRET_SETTINGS {
        settings.remove(*secret);
    }
    serde_json::to_string_pretty(&settings)
        .map_err(|err| ViewerControlError::internal(format!("Failed to serialize settings: {err}")))
}

/// Overwrites the keys of `target` with those of `patch`, recursing into nested objects.
///
/// A value of another type replaces the old one whole, which is how an enum changes variant.
fn merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => *target = patch,
    }
}

/// Checks that every value in `patch` is in `settings`.
fn check_applied(settings: &Value, patch: &Value, path: &str) -> Result<(), ViewerControlError> {
    match (settings, patch) {
        (Value::Object(settings), Value::Object(patch)) => {
            for (key, value) in patch {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                let Some(setting) = settings.get(key) else {
                    return Err(ViewerControlError::invalid_argument(format!(
                        "Unknown setting {path:?}. Call get_settings to see the available settings."
                    )));
                };
                check_applied(setting, value, &path)?;
            }
            Ok(())
        }
        (Value::Number(setting), Value::Number(value)) if same_number(setting, value) => Ok(()),
        (setting, value) if setting == value => Ok(()),
        _ => Err(ViewerControlError::invalid_argument(format!(
            "Invalid value for setting {path:?}: {patch} does not fit. \
             Use the form get_settings reports, e.g. {settings}."
        ))),
    }
}

/// Equal, allowing for the precision lost when an `f32` setting stores an `f64`.
fn same_number(a: &serde_json::Number, b: &serde_json::Number) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => {
            a == b || (a - b).abs() <= f64::from(f32::EPSILON) * a.abs().max(b.abs())
        }
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use re_log_channel::ViewerControlErrorCode;

    use super::*;

    type TestResult = Result<(), ViewerControlError>;

    fn set(app_options: &mut AppOptions, json: &str) -> Result<Value, ViewerControlError> {
        let response = set_settings(
            app_options,
            SetSettingsRequest {
                settings_json: json.to_owned(),
            },
        )?;
        serde_json::from_str(&response.settings_json)
            .map_err(|err| ViewerControlError::internal(err.to_string()))
    }

    fn assert_invalid(app_options: &mut AppOptions, json: &str, expected_message: &str) {
        let before = app_options.clone();
        let err = set(app_options, json).err();
        assert_eq!(
            err.as_ref().map(|err| err.code),
            Some(ViewerControlErrorCode::InvalidArgument),
            "for {json}"
        );
        let message = err.map(|err| err.message).unwrap_or_default();
        assert!(message.contains(expected_message), "for {json}: {message}");
        assert_eq!(*app_options, before, "for {json}");
    }

    #[test]
    fn get_hides_secrets() -> TestResult {
        let app_options = AppOptions {
            mapbox_access_token: "pk.MAPBOX_TOKEN".to_owned(),
            ..AppOptions::test()
        };
        let json = get_settings(&app_options)?.settings_json;
        assert!(json.contains("show_metrics"), "{json}");
        assert!(!json.contains("mapbox_access_token"), "{json}");
        assert!(!json.contains("MAPBOX_TOKEN"), "{json}");
        Ok(())
    }

    #[test]
    fn set_changes_only_the_given_keys_also_when_nested() -> TestResult {
        let mut app_options = AppOptions {
            mapbox_access_token: "secret".to_owned(),
            ..AppOptions::test()
        };
        app_options.show_metrics = false;
        app_options.video.allow_slow_av1_decoding = false;
        let mut expected = app_options.clone();
        expected.show_metrics = true;
        expected.video.allow_slow_av1_decoding = true;

        let after = set(
            &mut app_options,
            r#"{"show_metrics": true, "video": {"allow_slow_av1_decoding": true}}"#,
        )?;

        assert_eq!(app_options, expected);
        assert_eq!(after["video"]["allow_slow_av1_decoding"], true);
        assert_eq!(after.get("mapbox_access_token"), None);
        Ok(())
    }

    #[test]
    fn set_switches_enum_variants() -> TestResult {
        let mut app_options = AppOptions::test();
        for variant in [
            r#""Everything""#,
            r#"{"Similar": null}"#,
            r#""Required""#,
            r#"{"Similar": {"secs": 5, "nanos": 0}}"#,
        ] {
            let after = set(
                &mut app_options,
                &format!(r#"{{"max_fetch_stage": {variant}}}"#),
            )?;
            let expected: Value = serde_json::from_str(variant)
                .map_err(|err| ViewerControlError::internal(err.to_string()))?;
            assert_eq!(after["max_fetch_stage"], expected);
        }
        Ok(())
    }

    #[test]
    fn set_keeps_f32_values() -> TestResult {
        let mut app_options = AppOptions::test();
        set(&mut app_options, r#"{"warn_e2e_latency": 0.1}"#)?;
        assert_eq!(app_options.warn_e2e_latency, 0.1);
        Ok(())
    }

    #[test]
    fn read_only_settings_and_tiny_memory_limits_may_be_sent_back_unchanged() -> TestResult {
        let mut app_options = AppOptions::test();
        app_options.memory_limit = re_memory::MemoryLimit::ZERO;
        let before = app_options.clone();
        let everything = get_settings(&app_options)?.settings_json;

        set(&mut app_options, &everything)?;

        assert_eq!(app_options, before);
        Ok(())
    }

    #[test]
    fn memory_limit_floor() -> TestResult {
        let mut app_options = AppOptions::test();
        let floor = MIN_MEMORY_LIMIT_BYTES;

        assert_invalid(&mut app_options, r#"{"memory_limit": 0}"#, "memory_limit");
        let just_below = format!(r#"{{"memory_limit": {}}}"#, floor - 1);
        assert_invalid(&mut app_options, &just_below, "memory_limit");
        set(&mut app_options, &format!(r#"{{"memory_limit": {floor}}}"#))?;

        app_options.memory_limit = re_memory::MemoryLimit::from_bytes(1_000);
        set(&mut app_options, r#"{"memory_limit": 2000}"#)?;
        assert_eq!(app_options.memory_limit.as_bytes(), 2_000);
        assert_invalid(
            &mut app_options,
            r#"{"memory_limit": 1000}"#,
            "memory_limit",
        );

        set(&mut app_options, r#"{"memory_limit": null}"#)?;
        assert!(!app_options.memory_limit.is_limited());
        Ok(())
    }

    #[test]
    fn invalid_patches_change_nothing() {
        let mut app_options = AppOptions::test();
        assert_invalid(&mut app_options, "not json", "not valid JSON");
        assert_invalid(&mut app_options, "[]", "must be a JSON object");
        assert_invalid(
            &mut app_options,
            r#"{"show_metrics": true, "no_such": 1}"#,
            r#"Unknown setting "no_such""#,
        );
        assert_invalid(
            &mut app_options,
            r#"{"video": {"no_such": 1}}"#,
            r#"Unknown setting "video.no_such""#,
        );
        assert_invalid(
            &mut app_options,
            r#"{"video": {"allow_slow_av1_decoding": "yes"}}"#,
            r#""video.allow_slow_av1_decoding""#,
        );
        assert_invalid(&mut app_options, r#"{"video": null}"#, r#""video""#);
        assert_invalid(
            &mut app_options,
            r#"{"warn_e2e_latency": 1e39}"#,
            r#""warn_e2e_latency""#,
        );
        assert_invalid(
            &mut app_options,
            r#"{"mapbox_access_token": "x"}"#,
            "mapbox_access_token",
        );
    }

    #[test]
    fn read_only_settings_cannot_change() {
        let mut app_options = AppOptions::test();
        assert_invalid(
            &mut app_options,
            r#"{"cache_directory": "/tmp/elsewhere"}"#,
            r#""cache_directory""#,
        );
        assert_invalid(
            &mut app_options,
            r#"{"video": {"ffmpeg_path": "/bin/sh"}}"#,
            r#""video.ffmpeg_path""#,
        );
        let flipped = !app_options.video.override_ffmpeg_path;
        assert_invalid(
            &mut app_options,
            &format!(r#"{{"video": {{"override_ffmpeg_path": {flipped}}}}}"#),
            r#""video.override_ffmpeg_path""#,
        );
        assert_invalid(
            &mut app_options,
            r#"{"video": ["Auto", true, true, "/bin/sh"]}"#,
            r#""video""#,
        );
    }
}
