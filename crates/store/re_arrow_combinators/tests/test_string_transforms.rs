mod util;

use re_arrow_combinators::Transform as _;
use re_arrow_combinators::map::{MapList, StringPrefix, StringSuffix};
use re_arrow_combinators::reshape::GetField;

use crate::util::{DisplayRB, fixtures::nested_string_struct_column};

/// Tests that `StringPrefix` and `StringSuffix` work correctly when the `StringArray`
/// is extracted from a nested struct where string arrays share a common values buffer.
#[test]
fn test_string_transforms_from_nested_struct() -> Result<(), Box<dyn std::error::Error>> {
    let list_array = nested_string_struct_column();

    let names_list = MapList::new(GetField::new("data"))
        .then(MapList::new(GetField::new("names")))
        .transform(&list_array)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(names_list.clone()), @r"
    ┌──────────────────┐
    │ col              │
    │ ---              │
    │ type: List(Utf8) │
    ╞══════════════════╡
    │ [alice]          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, dave]     │
    └──────────────────┘
    ");

    let colors_list = MapList::new(GetField::new("data"))
        .then(MapList::new(GetField::new("colors")))
        .transform(&list_array)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(colors_list.clone()), @r"
    ┌──────────────────┐
    │ col              │
    │ ---              │
    │ type: List(Utf8) │
    ╞══════════════════╡
    │ [red]            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, yellow]   │
    └──────────────────┘
    ");

    // Test prefix on names array using MapList.
    let prefix_names = MapList::new(StringPrefix::new("user:"))
        .transform(&names_list)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(prefix_names.clone()), @r"
    ┌───────────────────┐
    │ col               │
    │ ---               │
    │ type: List(Utf8)  │
    ╞═══════════════════╡
    │ [user:alice]      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, user:dave] │
    └───────────────────┘
    ");

    // Test suffix on colors array using MapList.
    let suffix_colors = MapList::new(StringSuffix::new("_color"))
        .transform(&colors_list)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(suffix_colors.clone()), @r"
    ┌──────────────────────┐
    │ col                  │
    │ ---                  │
    │ type: List(Utf8)     │
    ╞══════════════════════╡
    │ [red_color]          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                 │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, yellow_color] │
    └──────────────────────┘
    ");

    // Test chaining on names array using MapList and Then (via .then()).
    let chained_names = MapList::new(StringPrefix::new("<").then(StringSuffix::new(">")))
        .transform(&names_list)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(chained_names.clone()), @r"
    ┌──────────────────┐
    │ col              │
    │ ---              │
    │ type: List(Utf8) │
    ╞══════════════════╡
    │ [<alice>]        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, <dave>]   │
    └──────────────────┘
    ");

    // Verify original nested list structure is unaffected by the transformations.
    insta::assert_snapshot!(DisplayRB(list_array.clone()), @r#"
    ┌───────────────────────────────────────────────────────────────────┐
    │ col                                                               │
    │ ---                                                               │
    │ type: List(Struct("data": Struct("names": Utf8, "colors": Utf8))) │
    ╞═══════════════════════════════════════════════════════════════════╡
    │ [{data: {names: alice, colors: red}}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{data: null}, {data: {names: dave, colors: yellow}}]             │
    └───────────────────────────────────────────────────────────────────┘
    "#);

    Ok(())
}

/// Tests that `StringPrefix` and `StringSuffix` preserve empty strings as-is when configured to do so.
#[test]
fn test_string_transforms_preserve_empty_strings() -> Result<(), Box<dyn std::error::Error>> {
    use arrow::array::StringArray;

    let input = StringArray::from(vec![Some("hello"), Some(""), None, Some("world")]);

    let prefixed = StringPrefix::new("prefix_")
        .with_prefix_empty_string(false)
        .transform(&input)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(prefixed), @r"
    ┌──────────────┐
    │ col          │
    │ ---          │
    │ type: Utf8   │
    ╞══════════════╡
    │ prefix_hello │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ prefix_world │
    └──────────────┘
    ");

    let suffixed = StringSuffix::new("_suffix")
        .with_suffix_empty_string(false)
        .transform(&input)?
        .unwrap();
    insta::assert_snapshot!(DisplayRB(suffixed), @r"
    ┌──────────────┐
    │ col          │
    │ ---          │
    │ type: Utf8   │
    ╞══════════════╡
    │ hello_suffix │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ world_suffix │
    └──────────────┘
    ");

    Ok(())
}
