mod util;

use std::sync::Arc;

use re_chunk::ArrowArray as _;
use re_lenses_core::{Selector, SelectorError as Error};

use crate::util::fixtures;

#[test]
fn execute_struct_field() -> Result<(), Error> {
    let array = fixtures::struct_column();

    let result = ".location"
        .parse::<Selector>()?
        .execute(Arc::new(array))?
        .unwrap();

    insta::assert_snapshot!(util::DisplayRB(result), @r#"
    ┌──────────────────────────────────────────┐
    │ col                                      │
    │ ---                                      │
    │ type: Struct("x": Float64, "y": Float64) │
    ╞══════════════════════════════════════════╡
    │ {x: 1.0, y: 2.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ {x: 3.0, y: 4.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ {x: 5.0, y: null}                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ {x: 7.0, y: 8.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    └──────────────────────────────────────────┘
    "#);

    Ok(())
}

#[test]
fn execute_each_struct_field() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    let result = ".[].location"
        .parse::<Selector>()?
        .execute(Arc::new(array.clone()))?
        .unwrap();

    // NOTE: When calling `execute` without a surrounding `map()`
    // statement it is possible to change the row count. This mimics
    // the behavior of `jq`.
    assert_eq!(array.len(), 7);
    assert_eq!(result.len(), 8);

    insta::assert_snapshot!(util::DisplayRB(result), @r#"
    ┌──────────────────────────────────────────┐
    │ col                                      │
    │ ---                                      │
    │ type: Struct("x": Float64, "y": Float64) │
    ╞══════════════════════════════════════════╡
    │ {x: 1.0, y: 2.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ {x: 3.0, y: 4.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ {x: 5.0, y: 6.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ {x: 7.0, y: 8.0}                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    └──────────────────────────────────────────┘
    "#);

    Ok(())
}
