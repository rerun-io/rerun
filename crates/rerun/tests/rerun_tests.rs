/// Regression test for checking that `RowId`s are generated in-order (when single-threaded).
///
/// Out-of-order row IDs is technically fine, but can cause unnecessary performance issues.
///
/// See for instance <https://github.com/rerun-io/rerun/issues/4415>.
#[test]
fn test_row_id_order() {
    let mut batcher_config = rerun::log::DataTableBatcherConfig::NEVER;
    batcher_config.hooks.on_insert = Some(std::sync::Arc::new(|rows| {
        if let [.., penultimate, ultimate] = rows {
            assert!(
                penultimate.row_id() <= ultimate.row_id(),
                "Rows coming to batcher out-of-order"
            );
        }
    }));
    let (rec, _mem_storage) = rerun::RecordingStreamBuilder::new("rerun_example_test")
        .batcher_config(batcher_config)
        .memory()
        .unwrap();

    for _ in 0..10 {
        rec.log(
            "foo",
            &rerun::Points2D::new([(1.0, 2.0), (3.0, 4.0)]).with_radii([1.0]),
        )
        .unwrap();
    }
}
