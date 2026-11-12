// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view to display any data in a tabular form.
///
/// Any data from the store can be shown, using a flexible, user-configurable query.
///
/// See [Dataframe queries](https://rerun.io/docs/query-and-transform/dataframes/dataframe-queries) to learn more about the query model.
///
/// \example views/dataframe title="Use a blueprint to customize a DataframeView." image="https://static.rerun.io/dataframe_view/f89ae330b04baaa9b7576765dce37b5d4e7cef4e/1200w.png" alt="Dataframe view of the /trig entities with cos, sin and tan_sparse scalar columns, and its query settings in the selection panel"
#[rerun::rerun_type]
#[docs(keywords = "table, tabular, dataframe, rows, columns, time range")]
#[docs(archetype_agnostic)]
#[rerun(view_identifier = "Dataframe")]
#[rerun(state = "unstable")]
pub struct DataframeView {
    /// Query of the dataframe.
    pub query: rerun::blueprint::archetypes::DataframeQuery,
}
