mod util;

use re_arrow_combinators::Transform as _;
use re_arrow_combinators::map::{MapList, StringPrefix, StringSuffix};
use re_arrow_combinators::reshape::GetField;

use crate::util::{DisplayRB, fixtures::nested_string_struct_column};

/// Tests that `StringPrefix` and `StringSuffix` work correctly when the `StringArray`
/// is extracted from a nested struct where string arrays share a common values buffer.
#[test]
fn test_string_transforms_from_nested_struct() {
    let list_array = nested_string_struct_column();

    let names_list = MapList::new(GetField::new("data"))
        .then(MapList::new(GetField::new("names")))
        .transform(&list_array)
        .expect("failed to extract names");
    insta::assert_snapshot!(DisplayRB(names_list.clone()), @r"
        ┌────────────────────────────────────┐
        │ col                                │
        │ ---                                │
        │ type: nullable List[nullable Utf8] │
        ╞════════════════════════════════════╡
        │ [alice]                            │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null, dave]                       │
        └────────────────────────────────────┘
        ");

    let colors_list = MapList::new(GetField::new("data"))
        .then(MapList::new(GetField::new("colors")))
        .transform(&list_array)
        .expect("failed to extract colors");
    insta::assert_snapshot!(DisplayRB(colors_list.clone()), @r"
        ┌────────────────────────────────────┐
        │ col                                │
        │ ---                                │
        │ type: nullable List[nullable Utf8] │
        ╞════════════════════════════════════╡
        │ [red]                              │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null, yellow]                     │
        └────────────────────────────────────┘
        ");

    // Test prefix on names array using MapList.
    let prefix_names = MapList::new(StringPrefix::new("user:"))
        .transform(&names_list)
        .expect("prefix transformation failed");
    insta::assert_snapshot!(DisplayRB(prefix_names.clone()), @r"
        ┌────────────────────────────────────┐
        │ col                                │
        │ ---                                │
        │ type: nullable List[nullable Utf8] │
        ╞════════════════════════════════════╡
        │ [user:alice]                       │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null, user:dave]                  │
        └────────────────────────────────────┘
        ");

    // Test suffix on colors array using MapList.
    let suffix_colors = MapList::new(StringSuffix::new("_color"))
        .transform(&colors_list)
        .expect("suffix transformation failed");
    insta::assert_snapshot!(DisplayRB(suffix_colors.clone()), @r"
        ┌────────────────────────────────────┐
        │ col                                │
        │ ---                                │
        │ type: nullable List[nullable Utf8] │
        ╞════════════════════════════════════╡
        │ [red_color]                        │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null, yellow_color]               │
        └────────────────────────────────────┘
        ");

    // Test chaining on names array using MapList and Compose (via .then()).
    let chained_names = MapList::new(StringPrefix::new("<").then(StringSuffix::new(">")))
        .transform(&names_list)
        .expect("chained transformation failed");
    insta::assert_snapshot!(DisplayRB(chained_names.clone()), @r"
        ┌────────────────────────────────────┐
        │ col                                │
        │ ---                                │
        │ type: nullable List[nullable Utf8] │
        ╞════════════════════════════════════╡
        │ [<alice>]                          │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null, <dave>]                     │
        └────────────────────────────────────┘
        ");

    // Verify original nested list structure is unaffected by the transformations.
    insta::assert_snapshot!(DisplayRB(list_array.clone()), @r"
        ┌───────────────────────────────────────────────────────┐
        │ col                                                   │
        │ ---                                                   │
        │ type: nullable List[nullable Struct[1]]               │
        ╞═══════════════════════════════════════════════════════╡
        │ [{data: {names: alice, colors: red}}]                 │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                                                │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                                                  │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{data: null}, {data: {names: dave, colors: yellow}}] │
        └───────────────────────────────────────────────────────┘
        ");
}

/// Tests that `StringPrefix` and `StringSuffix` preserve empty strings as-is when configured to do so.
#[test]
fn test_string_transforms_preserve_empty_strings() {
    use arrow::array::StringArray;

    let input = StringArray::from(vec![Some("hello"), Some(""), None, Some("world")]);

    let prefixed = StringPrefix::new("prefix_")
        .with_prefix_empty_string(false)
        .transform(&input)
        .unwrap();
    insta::assert_snapshot!(DisplayRB(prefixed), @r"
        ┌─────────────────────┐
        │ col                 │
        │ ---                 │
        │ type: nullable Utf8 │
        ╞═════════════════════╡
        │ prefix_hello        │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │                     │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ prefix_world        │
        └─────────────────────┘
        ");

    let suffixed = StringSuffix::new("_suffix")
        .with_suffix_empty_string(false)
        .transform(&input)
        .unwrap();
    insta::assert_snapshot!(DisplayRB(suffixed), @r"
        ┌─────────────────────┐
        │ col                 │
        │ ---                 │
        │ type: nullable Utf8 │
        ╞═════════════════════╡
        │ hello_suffix        │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │                     │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ world_suffix        │
        └─────────────────────┘
        ");
}
