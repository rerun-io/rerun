use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
use re_sdk_types::components::Visible;
use re_test_context::TestContext;
use re_viewer_context::{BlueprintContext as _, ContainerId, Contents, ViewerContext};

use crate::{ContainerBlueprint, ViewBlueprint, ViewContents, ViewportBlueprint, ViewportCommand};

fn edit<R>(test: &TestContext, edit: impl FnOnce(&ViewerContext<'_>, ViewportBlueprint) -> R) -> R {
    let result = test.run_once_in_egui_central_panel(|ctx, _| {
        edit(
            ctx,
            ViewportBlueprint::from_db(ctx.blueprint_db(), ctx.blueprint_query),
        )
    });
    test.handle_system_commands(&egui::Context::default());
    result
}

fn fixture() -> (TestContext, ContainerId) {
    let test = TestContext::new();
    let source = ContainerId::random();
    test.run_once_in_egui_central_panel(|ctx, _| {
        let mut views = (0..4)
            .map(|i| {
                let mut view = ViewBlueprint::new_with_root_wildcard("TestView".into());
                view.display_name = Some(format!("Plot {i}"));
                view.space_origin = "robot".into();
                view.titlebar = false;
                view.save_to_blueprint_store(ctx);
                ctx.save_blueprint_archetype(
                    ViewContents::base_override_path_for_entity(view.id, &"robot".into()),
                    &blueprint_archetypes::EntityBehavior::new().with_visible(false),
                );
                Contents::View(view.id)
            })
            .collect::<Vec<_>>();

        let horizontal = ContainerBlueprint {
            container_kind: egui_tiles::ContainerKind::Horizontal,
            display_name: Some("Uneven row".into()),
            contents: views.drain(..2).collect(),
            col_shares: vec![3.0, 1.0],
            ..Default::default()
        };

        let mut tabs = ContainerBlueprint::default();
        tabs.container_kind = egui_tiles::ContainerKind::Tabs;
        tabs.display_name = Some("Notes".into());
        tabs.contents = views;
        tabs.active_tab = Some(tabs.contents[1]);

        let empty = ContainerBlueprint {
            container_kind: egui_tiles::ContainerKind::Vertical,
            display_name: Some("Empty hidden container".into()),
            visible: false,
            ..Default::default()
        };

        let mut container = ContainerBlueprint::new(source);
        container.display_name = Some("Dashboard".into());
        container.contents = vec![
            Contents::Container(horizontal.id),
            Contents::Container(tabs.id),
            Contents::Container(empty.id),
        ];
        container.grid_columns = Some(2);
        container.col_shares = vec![2.0, 1.0];
        container.row_shares = vec![3.0, 1.0];

        let mut root = ContainerBlueprint::default();
        root.container_kind = egui_tiles::ContainerKind::Tabs;
        root.contents = vec![Contents::Container(source)];
        root.active_tab = Some(root.contents[0]);
        for container in [&horizontal, &tabs, &empty, &container, &root] {
            container.save_to_blueprint_store(ctx);
        }
        ctx.save_blueprint_archetype(
            crate::VIEWPORT_PATH.into(),
            &blueprint_archetypes::ViewportBlueprint::new()
                .with_root_container(root.id)
                .with_auto_layout(false)
                .with_auto_views(false),
        );
    });
    test.handle_system_commands(&egui::Context::default());
    (test, source)
}

fn assert_independent_copy(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    original: Contents,
    copied: Contents,
) {
    assert_ne!(original, copied);
    match (original, copied) {
        (Contents::Container(original), Contents::Container(copied)) => {
            let original = blueprint.container(&original).unwrap();
            let copied = blueprint.container(&copied).unwrap();
            assert_eq!(original.display_name, copied.display_name);
            assert_eq!(original.container_kind, copied.container_kind);
            assert_eq!(original.col_shares, copied.col_shares);
            assert_eq!(original.row_shares, copied.row_shares);
            assert_eq!(original.grid_columns, copied.grid_columns);
            assert_eq!(original.visible, copied.visible);
            assert_eq!(original.contents.len(), copied.contents.len());
            assert_eq!(
                original
                    .active_tab
                    .and_then(|active| original.contents.iter().position(|c| *c == active)),
                copied
                    .active_tab
                    .and_then(|active| copied.contents.iter().position(|c| *c == active)),
            );
            for (&original, &copied) in std::iter::zip(&original.contents, &copied.contents) {
                assert_independent_copy(ctx, blueprint, original, copied);
            }
        }
        (Contents::View(original), Contents::View(copied)) => {
            let original = blueprint.view(&original).unwrap();
            let copied = blueprint.view(&copied).unwrap();
            assert_eq!(original.display_name, copied.display_name);
            assert_eq!(original.space_origin, copied.space_origin);
            assert_eq!(original.class_identifier(), copied.class_identifier());
            assert_eq!(original.visible, copied.visible);
            assert_eq!(original.titlebar, copied.titlebar);
            assert_eq!(
                original.contents.entity_path_filter(),
                copied.contents.entity_path_filter()
            );
            assert_ne!(original.defaults_path, copied.defaults_path);
            assert_eq!(
                view_visible_override(ctx, original.id),
                view_visible_override(ctx, copied.id)
            );
        }
        _ => panic!("the copy changed the kind of a child"),
    }
}

#[test]
fn clone_nested_container_preserves_settings_and_independent_children() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    edit(&test, |ctx, blueprint| {
        assert_eq!(blueprint.views.len(), 8);
        assert_independent_copy(
            ctx,
            &blueprint,
            Contents::Container(source),
            Contents::Container(target),
        );
        let parent = blueprint.container(&blueprint.root_container).unwrap();
        assert_eq!(
            parent.contents,
            [Contents::Container(source), Contents::Container(target)]
        );
        assert_eq!(parent.active_tab, Some(Contents::Container(target)));
        blueprint
            .container(&target)
            .unwrap()
            .set_display_name(ctx, Some("Independent copy".into()));
        blueprint
            .container(&target)
            .unwrap()
            .set_visible(ctx, false);
    });
    edit(&test, |_, blueprint| {
        assert_eq!(
            blueprint
                .container(&source)
                .unwrap()
                .display_name
                .as_deref(),
            Some("Dashboard")
        );
        assert!(blueprint.container(&source).unwrap().visible);
        assert_eq!(
            blueprint
                .container(&target)
                .unwrap()
                .display_name
                .as_deref(),
            Some("Independent copy")
        );
        assert!(!blueprint.container(&target).unwrap().visible);
    });
}

#[test]
fn clone_root_keeps_original_and_copy_in_new_tabs() {
    let (test, _) = fixture();
    let (source, target) = edit(&test, |ctx, blueprint| {
        let source = blueprint.root_container;
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.save_to_blueprint_store(ctx);
        (source, target)
    });
    edit(&test, |ctx, blueprint| {
        assert_ne!(blueprint.root_container, source);
        assert_ne!(blueprint.root_container, target);
        let root = blueprint.container(&blueprint.root_container).unwrap();
        assert_eq!(root.container_kind, egui_tiles::ContainerKind::Tabs);
        assert_eq!(
            root.contents,
            [Contents::Container(source), Contents::Container(target)]
        );
        assert_independent_copy(
            ctx,
            &blueprint,
            Contents::Container(source),
            Contents::Container(target),
        );
    });
}

#[test]
fn missing_container_cannot_be_cloned() {
    let (test, _) = fixture();
    edit(&test, |ctx, blueprint| {
        assert!(
            blueprint
                .duplicate_container(&ContainerId::random(), ctx)
                .is_none()
        );
        assert!(blueprint.deferred_commands.lock().is_empty());
    });
}

#[test]
fn missing_descendant_does_not_insert_partial_copy() {
    let (test, source) = fixture();
    edit(&test, |ctx, mut blueprint| {
        let view = *blueprint.views.keys().next().unwrap();
        blueprint.views.remove(&view);
        let old_views = blueprint.views.len();
        let old_containers = blueprint.containers.len();
        super::duplicate::apply_duplicate_container(
            ctx,
            &mut blueprint,
            source,
            ContainerId::random(),
        );
        assert_eq!(blueprint.views.len(), old_views);
        assert_eq!(blueprint.containers.len(), old_containers);
    });
}

#[test]
fn clone_uses_preceding_layout_command() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        blueprint.set_container_kind(source, egui_tiles::ContainerKind::Horizontal);
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    edit(&test, |_, blueprint| {
        assert_eq!(
            blueprint.container(&source).unwrap().container_kind,
            egui_tiles::ContainerKind::Horizontal
        );
        assert_eq!(
            blueprint.container(&target).unwrap().container_kind,
            egui_tiles::ContainerKind::Horizontal
        );
    });
}

#[test]
fn clone_excludes_following_layout_command() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.set_container_kind(source, egui_tiles::ContainerKind::Horizontal);
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    edit(&test, |_, blueprint| {
        assert_eq!(
            blueprint.container(&source).unwrap().container_kind,
            egui_tiles::ContainerKind::Horizontal
        );
        assert_eq!(
            blueprint.container(&target).unwrap().container_kind,
            egui_tiles::ContainerKind::Grid
        );
    });
}

#[test]
fn clone_uses_preceding_moves_and_excludes_following_moves() {
    let (test, source) = fixture();
    let (target, removed, added) = edit(&test, |ctx, blueprint| {
        let row = blueprint.container(&source).unwrap().contents[0]
            .as_container_id()
            .unwrap();
        let removed = blueprint.container(&row).unwrap().contents[0];
        let added = blueprint.container(&row).unwrap().contents[1];
        // Move one child out before cloning, then another out after cloning.
        blueprint.move_contents(vec![removed], source, 0);
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.move_contents(vec![added], source, 1);
        blueprint.save_to_blueprint_store(ctx);
        (target, removed, added)
    });
    edit(&test, |_, blueprint| {
        let original = blueprint.container(&source).unwrap();
        assert_eq!(&original.contents[..2], &[removed, added]);
        let copied = blueprint.container(&target).unwrap();
        assert_eq!(copied.contents.len(), 4);
        let copied_row = blueprint
            .container(&copied.contents[1].as_container_id().unwrap())
            .unwrap();
        assert_eq!(copied_row.contents.len(), 1);
        let copied_view = blueprint
            .view(&copied_row.contents[0].as_view_id().unwrap())
            .unwrap();
        assert_eq!(copied_view.display_name.as_deref(), Some("Plot 1"));
        assert_ne!(copied_row.contents[0], added);
    });
}

#[test]
fn clone_uses_preceding_share_and_tab_changes() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        let children = &blueprint.container(&source).unwrap().contents;
        let row = children[0].as_container_id().unwrap();
        let tabs = children[1].as_container_id().unwrap();
        let first_tab = blueprint.container(&tabs).unwrap().contents[0]
            .as_view_id()
            .unwrap();
        blueprint.make_all_children_same_size(&row);
        blueprint.focus_tab(first_tab);
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        // Later changes must not change what was captured by Clone.
        blueprint.focus_tab(
            blueprint.container(&tabs).unwrap().contents[1]
                .as_view_id()
                .unwrap(),
        );
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    edit(&test, |_, blueprint| {
        let copied = blueprint.container(&target).unwrap();
        let row = blueprint
            .container(&copied.contents[0].as_container_id().unwrap())
            .unwrap();
        assert_eq!(row.col_shares, [1.0, 1.0]);
        let tabs = blueprint
            .container(&copied.contents[1].as_container_id().unwrap())
            .unwrap();
        assert_eq!(tabs.active_tab, Some(tabs.contents[0]));
        let original_tabs = blueprint
            .container(
                &blueprint.container(&source).unwrap().contents[1]
                    .as_container_id()
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(original_tabs.active_tab, Some(original_tabs.contents[1]));
    });
}

#[test]
fn clone_includes_view_and_new_container_added_in_same_batch() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        let mut view = ViewBlueprint::new_with_root_wildcard("TestView".into());
        view.display_name = Some("New view".into());
        blueprint.add_views(std::iter::once(view), Some(source), Some(0));
        let row = blueprint.container(&source).unwrap().contents[0]
            .as_container_id()
            .unwrap();
        let moved = blueprint.container(&row).unwrap().contents.clone();
        blueprint.move_contents_to_new_container(
            moved,
            egui_tiles::ContainerKind::Vertical,
            source,
            1,
        );
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    edit(&test, |ctx, blueprint| {
        assert_independent_copy(
            ctx,
            &blueprint,
            Contents::Container(source),
            Contents::Container(target),
        );
        let copied = blueprint.container(&target).unwrap();
        let new_view = blueprint
            .view(&copied.contents[0].as_view_id().unwrap())
            .unwrap();
        assert_eq!(new_view.display_name.as_deref(), Some("New view"));
        let nested = blueprint
            .container(&copied.contents[1].as_container_id().unwrap())
            .unwrap();
        assert_eq!(nested.container_kind, egui_tiles::ContainerKind::Vertical);
        assert_eq!(nested.contents.len(), 2);
    });
}

#[test]
fn clone_inserts_after_source_in_each_parent_layout() {
    for kind in egui_tiles::ContainerKind::ALL {
        let (test, source) = fixture();
        let siblings = edit(&test, |ctx, blueprint| {
            let before = ViewBlueprint::new_with_root_wildcard("TestView".into());
            let after = ViewBlueprint::new_with_root_wildcard("TestView".into());
            before.save_to_blueprint_store(ctx);
            after.save_to_blueprint_store(ctx);
            let siblings = [Contents::View(before.id), Contents::View(after.id)];
            let mut root = blueprint
                .container(&blueprint.root_container)
                .unwrap()
                .clone();
            root.container_kind = kind;
            root.contents = vec![siblings[0], Contents::Container(source), siblings[1]];
            root.col_shares = vec![2.0, 3.0, 5.0];
            root.row_shares = vec![2.0, 3.0, 5.0];
            root.grid_columns = Some(3);
            root.save_to_blueprint_store(ctx);
            siblings
        });
        let target = edit(&test, |ctx, blueprint| {
            let target = blueprint.duplicate_container(&source, ctx).unwrap();
            blueprint.save_to_blueprint_store(ctx);
            target
        });
        edit(&test, |ctx, blueprint| {
            let root = blueprint.container(&blueprint.root_container).unwrap();
            assert_eq!(root.container_kind, kind);
            assert_eq!(
                root.contents,
                [
                    siblings[0],
                    Contents::Container(source),
                    Contents::Container(target),
                    siblings[1]
                ]
            );
            match kind {
                egui_tiles::ContainerKind::Horizontal => {
                    assert_eq!(root.col_shares[0], 2.0);
                    assert_eq!(root.col_shares[1], 3.0);
                    assert_eq!(root.col_shares[3], 5.0);
                }
                egui_tiles::ContainerKind::Vertical => {
                    assert_eq!(root.row_shares[0], 2.0);
                    assert_eq!(root.row_shares[1], 3.0);
                    assert_eq!(root.row_shares[3], 5.0);
                }
                egui_tiles::ContainerKind::Grid => {
                    assert_eq!(root.grid_columns, Some(3));
                    assert_eq!(root.col_shares, [2.0, 3.0, 5.0]);
                    assert_eq!(root.row_shares, [2.0, 3.0, 5.0]);
                }
                egui_tiles::ContainerKind::Tabs => {
                    assert_eq!(root.active_tab, Some(Contents::Container(target)));
                }
            }
            assert_independent_copy(
                ctx,
                &blueprint,
                Contents::Container(source),
                Contents::Container(target),
            );
        });
    }
}

fn view_visible_override(ctx: &ViewerContext<'_>, view: re_viewer_context::ViewId) -> Option<bool> {
    let results = ctx.blueprint_db().storage_engine().cache().latest_at(
        re_chunk_store::ChunkTrackingMode::Report,
        ctx.blueprint_query,
        &ViewContents::base_override_path_for_entity(view, &"robot".into()),
        [blueprint_archetypes::EntityBehavior::descriptor_visible().component],
    );
    results
        .component_mono::<Visible>(
            blueprint_archetypes::EntityBehavior::descriptor_visible().component,
        )
        .map(|visible| **visible)
}

#[test]
fn copied_view_override_can_change_without_affecting_original() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    let (original_view, copied_view) = edit(&test, |ctx, blueprint| {
        let first_view = |container| {
            let row = blueprint.container(&container).unwrap().contents[0]
                .as_container_id()
                .unwrap();
            blueprint.container(&row).unwrap().contents[0]
                .as_view_id()
                .unwrap()
        };
        let original_view = first_view(source);
        let copied_view = first_view(target);
        assert_eq!(view_visible_override(ctx, original_view), Some(false));
        assert_eq!(view_visible_override(ctx, copied_view), Some(false));
        ctx.save_blueprint_archetype(
            ViewContents::base_override_path_for_entity(copied_view, &"robot".into()),
            &blueprint_archetypes::EntityBehavior::new().with_visible(true),
        );
        (original_view, copied_view)
    });
    edit(&test, |ctx, _| {
        assert_eq!(view_visible_override(ctx, original_view), Some(false));
        assert_eq!(view_visible_override(ctx, copied_view), Some(true));
    });
}

#[test]
fn clone_uses_replaced_tree_and_temporary_tabs() {
    let (test, source) = fixture();
    let target = edit(&test, |ctx, blueprint| {
        let mut tree = blueprint.tree.clone();
        tree.simplify(&crate::tree_simplification_options());
        tree.set_visible(Contents::Container(source).as_tile_id(), false);
        blueprint
            .deferred_commands
            .lock()
            .push(ViewportCommand::SetTree(tree));
        let target = blueprint.duplicate_container(&source, ctx).unwrap();
        blueprint.save_to_blueprint_store(ctx);
        target
    });
    edit(&test, |ctx, blueprint| {
        assert!(!blueprint.container(&source).unwrap().visible);
        assert!(!blueprint.container(&target).unwrap().visible);
        assert_independent_copy(
            ctx,
            &blueprint,
            Contents::Container(source),
            Contents::Container(target),
        );
    });
}
