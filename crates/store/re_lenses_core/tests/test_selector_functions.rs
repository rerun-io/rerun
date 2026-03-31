#![expect(clippy::unwrap_used)] // Okay to use unwrap in tests

mod util;

use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, Float64Array, ListArray, StringArray};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::datatypes::{DataType, Field, Float64Type};

use re_lenses_core::combinators::Error;
use re_lenses_core::function_registry::{FunctionRegistry, FunctionRegistryError};
use re_lenses_core::{Literal, Runtime, Selector, SelectorError};
use util::DisplayRB;

// -- Transform functions -----------------------------------------------------

/// Doubles every float64 value.
fn double_values(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let values = source
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            expected: "Float64".into(),
            actual: source.data_type().clone(),
            context: "double_values".into(),
        })?;

    let doubled: Float64Array = values.iter().map(|v| v.map(|x| x * 2.0)).collect();
    Ok(Some(Arc::new(doubled)))
}

/// Repeats every float64 value 3 times, producing a list array.
fn repeat3(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let values = source
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            expected: "Float64".into(),
            actual: source.data_type().clone(),
            context: "repeat3".into(),
        })?;

    let repeated: ListArray = ListArray::from_iter_primitive::<Float64Type, _, _>(
        values
            .iter()
            .map(|v| Some(std::iter::repeat_n(v, 3).collect::<Vec<_>>())),
    );
    Ok(Some(Arc::new(repeated)))
}

/// Prepends a prefix to every string value.
fn prepend(prefix: String) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> {
    move |source: &ArrayRef| {
        let values = source
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "Utf8".into(),
                actual: source.data_type().clone(),
                context: "prepend".into(),
            })?;

        let prefixed: StringArray = values
            .iter()
            .map(|v| v.map(|s| format!("{prefix}{s}")))
            .collect();
        Ok(Some(Arc::new(prefixed) as ArrayRef))
    }
}

/// Replaces Float64 values > 4.0 with `null`, passes others through.
fn nullify_gt4(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let values = source
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            expected: "Float64".into(),
            actual: source.data_type().clone(),
            context: "nullify_gt4".into(),
        })?;

    let result: Float64Array = values
        .iter()
        .map(|v| {
            let x = v?;
            if x <= 4.0 { Some(x) } else { None }
        })
        .collect();
    Ok(Some(Arc::new(result)))
}

// -- Helpers -----------------------------------------------------------------

fn make_float_list(rows: &[Option<&[f64]>]) -> ListArray {
    ListArray::from_iter_primitive::<Float64Type, _, _>(
        rows.iter()
            .map(|row| row.map(|vals| vals.iter().map(|&v| Some(v)))),
    )
}

fn make_string_list(rows: &[Option<&[Option<&str>]>]) -> ListArray {
    let mut values: Vec<Option<&str>> = Vec::new();
    let mut offsets = vec![0i32];
    let mut nulls = Vec::new();

    for row in rows {
        if let Some(vals) = row {
            values.extend_from_slice(vals);
            offsets.push(values.len().try_into().unwrap());
            nulls.push(true);
        } else {
            offsets.push(*offsets.last().unwrap());
            nulls.push(false);
        }
    }

    let string_array = StringArray::from(values);
    ListArray::new(
        Arc::new(Field::new_list_field(DataType::Utf8, true)),
        OffsetBuffer::new(offsets.into()),
        Arc::new(string_array),
        Some(NullBuffer::from(nulls)),
    )
}

fn test_runtime() -> Runtime {
    let mut registry = FunctionRegistry::new();
    registry.register("double", || double_values).unwrap();
    registry.register("repeat3", || repeat3).unwrap();
    registry.register("prepend", prepend).unwrap();
    registry.register("nullify_gt4", || nullify_gt4).unwrap();

    Runtime {
        function_registry: Arc::new(registry),
    }
}

// -- Registry unit tests -----------------------------------------------------

#[test]
fn register_and_get_no_args() {
    let runtime = test_runtime();

    assert!(runtime.function_registry.get("double", &[]).is_ok());
}

#[test]
fn register_and_get_with_args() {
    let runtime = test_runtime();

    assert!(
        runtime
            .function_registry
            .get("prepend", &[Literal::String("hello_".into())])
            .is_ok()
    );
}

#[test]
fn get_unknown_function() {
    let registry = FunctionRegistry::new();
    let result = registry.get("nonexistent", &[]);
    assert!(matches!(
        result,
        Err(FunctionRegistryError::UnknownFunction { .. })
    ));
}

#[test]
fn get_no_arg_function_with_extra_args() {
    let runtime = test_runtime();

    // The zero-arg constructor ignores extra arguments, so this still succeeds.
    // This test documents the current behavior.
    let result = runtime
        .function_registry
        .get("double", &[Literal::String("unexpected".into())]);
    assert!(result.is_ok());
}

#[test]
fn get_one_arg_function_with_no_args() {
    let runtime = test_runtime();

    let result = runtime.function_registry.get("prepend", &[]);
    assert!(matches!(
        result,
        Err(FunctionRegistryError::WrongArguments { .. })
    ));
}

#[test]
fn register_multiple_functions() {
    let runtime = test_runtime();

    assert!(runtime.function_registry.get("double", &[]).is_ok());
    assert!(
        runtime
            .function_registry
            .get("prepend", &[Literal::String("x_".into())])
            .is_ok()
    );
}

// -- Selector + function integration tests -----------------------------------

#[test]
fn selector_calls_no_arg_function() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = make_float_list(&[Some(&[1.0, 2.0]), Some(&[3.0]), None]);

    let via_registry = Selector::parse("double()")?
        .with_runtime(runtime)
        .execute_per_row(&array)?
        .unwrap();

    let via_pipe = Selector::parse(".")?
        .pipe(double_values)
        .execute_per_row(&array)?
        .unwrap();

    assert_eq!(via_registry, via_pipe);

    insta::assert_snapshot!(DisplayRB(via_registry), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [2.0, 4.0]          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [6.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    └─────────────────────┘
    ");
    Ok(())
}

#[test]
fn selector_calls_function_with_string_arg() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = make_string_list(&[
        Some(&[Some("alice"), Some("bob")]),
        Some(&[Some("carol"), None]),
        None,
    ]);

    let result = Selector::parse(r#"prepend("hello_")"#)?
        .with_runtime(runtime)
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @r"
    ┌──────────────────────────┐
    │ col                      │
    │ ---                      │
    │ type: List(Utf8)         │
    ╞══════════════════════════╡
    │ [hello_alice, hello_bob] │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [hello_carol, null]      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                     │
    └──────────────────────────┘
    ");
    Ok(())
}

#[test]
fn selector_pipes_path_into_function() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = util::fixtures::nested_struct_column();

    let via_registry = ".location.x | double()"
        .parse::<Selector>()?
        .with_runtime(runtime.clone())
        .execute_per_row(&array)?
        .unwrap();

    let via_pipe = Selector::parse(".location.x")?
        .pipe(double_values)
        .execute_per_row(&array)?
        .unwrap();

    assert_eq!(via_registry, via_pipe);

    insta::assert_snapshot!(DisplayRB(via_registry), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [2.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [6.0, 10.0]         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, 14.0]        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null]        │
    └─────────────────────┘
    ");

    // NOTE: We also test functions that insert null values.
    let via_registry = ".location.x | nullify_gt4() | double()"
        .parse::<Selector>()?
        .with_runtime(runtime)
        .execute_per_row(&array)?
        .unwrap();

    let via_pipe = Selector::parse(".location.x")?
        .pipe(nullify_gt4)
        .pipe(double_values)
        .execute_per_row(&array)?
        .unwrap();

    assert_eq!(via_registry, via_pipe);

    insta::assert_snapshot!(DisplayRB(via_registry), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [2.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [6.0, null]         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null]        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null]        │
    └─────────────────────┘
    ");

    Ok(())
}

#[test]
fn selector_pipes_nested_list_path_into_function() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = util::fixtures::nested_list_struct_column();

    let via_registry = ".poses[].y | double()"
        .parse::<Selector>()?
        .with_runtime(runtime.clone())
        .execute_per_row(&array)?
        .unwrap();

    let via_pipe = Selector::parse(".poses[].y")?
        .pipe(double_values)
        .execute_per_row(&array)?
        .unwrap();

    assert_eq!(via_registry, via_pipe);

    insta::assert_snapshot!(DisplayRB(via_registry), @"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [4.0, 8.0]          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [12.0]              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, 20.0]        │
    └─────────────────────┘
    ");

    let result = ".poses[].y | double | repeat3"
        .parse::<Selector>()?
        .with_runtime(runtime.clone())
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @"
    ┌──────────────────────────────────────────┐
    │ col                                      │
    │ ---                                      │
    │ type: List(List(Float64))                │
    ╞══════════════════════════════════════════╡
    │ [[4.0, 4.0, 4.0], [8.0, 8.0, 8.0]]       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[12.0, 12.0, 12.0]]                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[null, null, null], [20.0, 20.0, 20.0]] │
    └──────────────────────────────────────────┘
    ");

    let result = ".poses[].y | double | repeat3 | .[]"
        .parse::<Selector>()?
        .with_runtime(runtime.clone())
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @"
    ┌──────────────────────────────────────┐
    │ col                                  │
    │ ---                                  │
    │ type: List(Float64)                  │
    ╞══════════════════════════════════════╡
    │ [4.0, 4.0, 4.0, 8.0, 8.0, 8.0]       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [12.0, 12.0, 12.0]                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                 │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null, null, 20.0, 20.0, 20.0] │
    └──────────────────────────────────────┘
    ");

    let result = "map(.poses[].y | double | repeat3 | .[])"
        .parse::<Selector>()?
        .with_runtime(runtime.clone())
        .execute(Arc::new(array.clone()))?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @"
    ┌──────────────────────────────────────┐
    │ col                                  │
    │ ---                                  │
    │ type: List(Float64)                  │
    ╞══════════════════════════════════════╡
    │ [4.0, 4.0, 4.0, 8.0, 8.0, 8.0]       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [12.0, 12.0, 12.0]                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                 │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null, null, 20.0, 20.0, 20.0] │
    └──────────────────────────────────────┘
    ");

    Ok(())
}

#[test]
fn selector_pipes_path_into_string_function() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = util::fixtures::nested_string_struct_column();

    let result = r#".data.names | prepend("user_")"#
        .parse::<Selector>()?
        .with_runtime(runtime)
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @r"
    ┌───────────────────┐
    │ col               │
    │ ---               │
    │ type: List(Utf8)  │
    ╞═══════════════════╡
    │ [user_alice]      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, user_dave] │
    └───────────────────┘
    ");
    Ok(())
}

#[test]
fn selector_chains_two_functions() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = make_string_list(&[Some(&[Some("world"), Some("there")]), Some(&[Some("x")])]);

    let via_registry = r#"prepend("hello_") | prepend("say_")"#
        .parse::<Selector>()?
        .with_runtime(runtime)
        .execute_per_row(&array)?
        .unwrap();

    let via_pipe = Selector::parse(".")?
        .pipe(prepend("hello_".into()))
        .pipe(prepend("say_".into()))
        .execute_per_row(&array)?
        .unwrap();

    assert_eq!(via_registry, via_pipe);

    insta::assert_snapshot!(DisplayRB(via_registry), @r"
    ┌────────────────────────────────────┐
    │ col                                │
    │ ---                                │
    │ type: List(Utf8)                   │
    ╞════════════════════════════════════╡
    │ [say_hello_world, say_hello_there] │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [say_hello_x]                      │
    └────────────────────────────────────┘
    ");
    Ok(())
}

#[test]
fn selector_unknown_function_errors() {
    let array = make_float_list(&[Some(&[1.0])]);

    let result = "missing_func()"
        .parse::<Selector>()
        .unwrap()
        .execute_per_row(&array);

    assert!(result.is_err());
}

#[test]
fn selector_pipes_struct_field_into_function() -> Result<(), SelectorError> {
    let runtime = test_runtime();

    let array = util::fixtures::struct_column();

    let via_registry = ".location.y | double()"
        .parse::<Selector>()?
        .with_runtime(runtime)
        .execute(Arc::new(array.clone()))?
        .unwrap();

    let via_pipe = Selector::parse(".location.y")?
        .pipe(double_values)
        .execute(Arc::new(array))?
        .unwrap();

    assert_eq!(via_registry.as_ref(), via_pipe.as_ref());

    insta::assert_snapshot!(util::DisplayRB(via_registry), @"
    ┌───────────────┐
    │ col           │
    │ ---           │
    │ type: Float64 │
    ╞═══════════════╡
    │ 4.0           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 8.0           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 16.0          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null          │
    └───────────────┘
    ");
    Ok(())
}

#[test]
fn selector_function_with_missing_args_errors() {
    let runtime = test_runtime();

    let array = make_string_list(&[Some(&[Some("hello")])]);

    let result = "prepend()"
        .parse::<Selector>()
        .unwrap()
        .with_runtime(runtime)
        .execute_per_row(&array);

    assert!(result.is_err());
}

#[test]
fn selector_deep_nested_list_double() -> Result<(), SelectorError> {
    let runtime = test_runtime();
    let array = util::fixtures::deep_nested_list_column();

    let result = ".[] | .[] | double()"
        .parse::<Selector>()?
        .with_runtime(runtime.clone())
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @r"
    ┌────────────────────────┐
    │ col                    │
    │ ---                    │
    │ type: List(Float64)    │
    ╞════════════════════════╡
    │ [2.0, 6.0, 10.0, 14.0] │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [18.0]                 │
    └────────────────────────┘
    ");

    let via_registry = "map(.[] | .[] | double())"
        .parse::<Selector>()?
        .with_runtime(runtime)
        .execute(Arc::new(array))?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(via_registry), @r"
    ┌────────────────────────┐
    │ col                    │
    │ ---                    │
    │ type: List(Float64)    │
    ╞════════════════════════╡
    │ [2.0, 6.0, 10.0, 14.0] │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [18.0]                 │
    └────────────────────────┘
    ");

    Ok(())
}

#[test]
fn selector_deep_nested_flatten_all() -> Result<(), SelectorError> {
    let array = util::fixtures::deep_nested_list_column();

    let result = ".[] | .[] | .[]"
        .parse::<Selector>()?
        .execute(Arc::new(array))?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @r"
    ┌───────────────┐
    │ col           │
    │ ---           │
    │ type: Float64 │
    ╞═══════════════╡
    │ 1.0           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 3.0           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 5.0           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 7.0           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 9.0           │
    └───────────────┘
    ");

    Ok(())
}
