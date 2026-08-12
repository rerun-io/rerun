//! Tests for `RecordingStream::log_file_*`.

#![cfg(feature = "importers")]

use re_sdk::{RecordingStreamBuilder, StoreId, StoreKind, log::LogMsg};
use re_sdk_types::archetypes::TextDocument;

const CURRENT_APPLICATION: &str = "rerun_example_current_application";
const UNRELATED_APPLICATION: &str = "rerun_example_unrelated_blueprint_application";

/// Writes a blueprint `.rbl` belonging to [`UNRELATED_APPLICATION`], and returns its store ID.
fn save_unrelated_blueprint(path: &std::path::Path) -> StoreId {
    let blueprint = RecordingStreamBuilder::new(UNRELATED_APPLICATION)
        .blueprint()
        .save(path)
        .expect("blueprint stream should be created");

    let blueprint_id = blueprint
        .store_info()
        .expect("blueprint stream should be enabled")
        .store_id
        .clone();
    assert!(blueprint_id.is_blueprint());

    blueprint
        .log_static("blueprint/entity", &TextDocument::new("blueprint data"))
        .expect("blueprint entity should log");

    // Activation is meant to arrive after all of the blueprint's data. `log_static` goes through
    // the batcher while `record_msg` does not, so without flushing here the two could be
    // serialized in either order.
    blueprint
        .flush_blocking()
        .expect("blueprint stream should flush");
    blueprint.record_msg(LogMsg::BlueprintActivationCommand(
        re_sdk::external::re_log_types::BlueprintActivationCommand::make_active(
            blueprint_id.clone(),
        ),
    ));

    // Dropping the stream flushes it and finalizes the file.
    drop(blueprint);

    blueprint_id
}

/// `log_file_from_path` must retarget an imported blueprint onto the current recording's
/// application, no matter which application the blueprint was saved under. Otherwise the viewer
/// would file the blueprint under a foreign application and never apply it.
#[test]
fn log_file_from_path_retargets_blueprint_to_current_application() {
    re_log::setup_logging();

    let rbl_file = tempfile::Builder::new()
        .suffix(".rbl")
        .tempfile()
        .expect("temp file should be created");
    let source_blueprint_id = save_unrelated_blueprint(rbl_file.path());

    let (rec, storage) = RecordingStreamBuilder::new(CURRENT_APPLICATION)
        .memory()
        .expect("recording stream should be created");
    let current_store_id = rec
        .store_info()
        .expect("recording stream should be enabled")
        .store_id
        .clone();

    rec.log_file_from_path(rbl_file.path(), None, false)
        .expect("blueprint file should be logged");

    // `take` flushes the recording stream, which in turn joins the importer threads, so every
    // imported message has landed by the time this returns.
    let imported = storage.take();

    let blueprint_messages = imported
        .iter()
        .filter(|msg| msg.store_id().kind() == StoreKind::Blueprint)
        .collect::<Vec<_>>();

    // Each `LogMsg` variant carries its store ID in a different place, and the importer patches
    // each one separately, so all three have to be present for this test to mean anything.
    // Counting them is deliberately avoided: the SDK also emits `/__properties` chunks, and how
    // many is none of this test's business.
    assert!(
        blueprint_messages
            .iter()
            .any(|msg| matches!(msg, LogMsg::SetStoreInfo(_))),
        "expected a blueprint `SetStoreInfo`, got {imported:#?}"
    );
    assert!(
        blueprint_messages
            .iter()
            .any(|msg| matches!(msg, LogMsg::ArrowMsg(..))),
        "expected a blueprint `ArrowMsg`, got {imported:#?}"
    );
    assert!(
        blueprint_messages
            .iter()
            .any(|msg| matches!(msg, LogMsg::BlueprintActivationCommand(_))),
        "expected a `BlueprintActivationCommand`, got {imported:#?}"
    );

    for blueprint_store_id in blueprint_messages.into_iter().map(LogMsg::store_id) {
        assert_eq!(
            blueprint_store_id.application_id(),
            current_store_id.application_id(),
            "the blueprint should have been retargeted onto the current application"
        );
        assert_eq!(
            blueprint_store_id.recording_id(),
            source_blueprint_id.recording_id(),
            "only the application ID should be patched, never the recording ID"
        );
    }
}
