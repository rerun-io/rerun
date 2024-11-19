from __future__ import annotations

import rerun.blueprint.components as components
import rerun.blueprint.datatypes as datatypes


def test_utf8list() -> None:
    # An array of string should be interpreted as a batch of a single Utf8List, which contains the provided strings.
    list_with_two_strings = ["String One", "String Two"]
    list_of_list_with_two_strings = [list_with_two_strings]
    assert (
        datatypes.Utf8ListBatch(list_with_two_strings).as_arrow_array()
        == datatypes.Utf8ListBatch(list_of_list_with_two_strings).as_arrow_array()
    )

    # A single Utf8List, an array of a single Utf8List should be the same as above
    single_utf8list = datatypes.Utf8List(["String One", "String Two"])
    list_with_single_utf8list = [single_utf8list]

    assert (
        datatypes.Utf8ListBatch(list_with_two_strings).as_arrow_array()
        == datatypes.Utf8ListBatch(single_utf8list).as_arrow_array()
    )
    assert (
        datatypes.Utf8ListBatch(single_utf8list).as_arrow_array()
        == datatypes.Utf8ListBatch(list_with_single_utf8list).as_arrow_array()
    )

    # A list of string arrays should be interpreted as a list of Utf8List
    two_string_arrays = [["1.1", "1.2"], ["2.1", "2.2"]]
    two_utf8_arrays = [datatypes.Utf8List(["1.1", "1.2"]), datatypes.Utf8List(["2.1", "2.2"])]
    assert (
        datatypes.Utf8ListBatch(two_string_arrays).as_arrow_array()
        == datatypes.Utf8ListBatch(two_utf8_arrays).as_arrow_array()
    )

    # A single string should be interpreted as a batch of a single Utf8List, which contains a single string
    single_string = "Hello"
    array_of_single_string = [single_string]
    array_of_array_of_single_string = [array_of_single_string]

    assert (
        datatypes.Utf8ListBatch(single_string).as_arrow_array()
        == datatypes.Utf8ListBatch(array_of_single_string).as_arrow_array()
    )

    assert (
        datatypes.Utf8ListBatch(array_of_single_string).as_arrow_array()
        == datatypes.Utf8ListBatch(array_of_array_of_single_string).as_arrow_array()
    )

    # A component delegating through to the underlying datatype should behave the same
    assert (
        components.VisualizerOverrides(single_string).as_arrow_array()
        == datatypes.Utf8ListBatch(array_of_array_of_single_string).as_arrow_array()
    )

    assert (
        components.VisualizerOverrides(list_with_two_strings).as_arrow_array()
        == datatypes.Utf8ListBatch(list_of_list_with_two_strings).as_arrow_array()
    )
