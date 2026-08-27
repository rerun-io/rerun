//! In-process integration tests driven through the `egui_kittest` viewer harness (`HarnessExt`).
//!
//! The out-of-process `InspectionHarness` suite lives in a separate test binary (`inspection/`) so
//! the two harnesses don't get mixed in one test.

mod add_entity_to_view_test;
mod add_visualizer_test;
mod assets;
mod basic_tests;
mod blueprint_context_menu_test;
mod blueprint_import_test;
mod cards_view_flagging;
mod check_focus_test;
mod container_context_menu_test;
mod context_menu_test;
mod dataset_folders;
mod deselect_on_escape;
mod drag_and_drop_selection;
mod drop_component_to_state_timeline_view;
mod drop_stream_to_view;
mod heuristics_mixed_2d_and_3d_test;
mod heuristics_mixed_all_root_test;
mod internal_catalog;
mod multi_container_test;
mod no_blueprint_test;
mod origin_heuristics_test;
mod parallelism_caching_reentrancy;
mod preview_table;
mod redap_catalog_select;
mod rrd_bw_compat_test;
mod source_component_test;
mod spatial_cross_view_interaction;
mod state_timeline_hover_highlight;
mod undo_redo_test;
mod view_defaults_test;
mod view_visualizers_test;
mod viewer_events_test;
mod views_spawned_test;
mod visualizer_instruction_errors_test;
mod watch_events;
