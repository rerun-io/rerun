//! Independent copies of configured container hierarchies.

use std::collections::BTreeMap;

use re_viewer_context::{ContainerId, Contents, ViewerContext};

use crate::{ContainerBlueprint, ViewBlueprint, ViewportBlueprint, ViewportCommand};

impl ViewportBlueprint {
    /// Duplicate a container and all its descendants, inserting the copy next to the original.
    ///
    /// Cloning the root puts the original and the copy in a new tab container.
    /// The operation is deferred until the end of the frame, like other layout edits.
    /// The copy includes layout commands preceding it, but not commands following it.
    pub fn duplicate_container(
        &self,
        container_id: &ContainerId,
        ctx: &ViewerContext<'_>,
    ) -> Option<ContainerId> {
        self.container(container_id)?;
        let new_id = ContainerId::random();
        self.enqueue_command(ViewportCommand::DuplicateContainer {
            source: *container_id,
            target: new_id,
        });
        self.mark_user_interaction(ctx);
        Some(new_id)
    }

    fn duplicate_container_tree(
        &self,
        current_containers: &BTreeMap<ContainerId, ContainerBlueprint>,
        source: ContainerId,
        target: ContainerId,
        ctx: &ViewerContext<'_>,
        containers: &mut Vec<ContainerBlueprint>,
        views: &mut Vec<ViewBlueprint>,
    ) -> Option<()> {
        let original = current_containers.get(&source)?;
        let mut container = original.clone();
        container.id = target;
        container.contents.clear();
        container.active_tab = None;
        for &child in &original.contents {
            let copied = match child {
                Contents::View(id) => {
                    let view = self
                        .view(&id)?
                        .duplicate(ctx.store_context, ctx.blueprint_query);
                    let new_id = view.id;
                    views.push(view);
                    Contents::View(new_id)
                }
                Contents::Container(id) => {
                    let new_id = ContainerId::random();
                    self.duplicate_container_tree(
                        current_containers,
                        id,
                        new_id,
                        ctx,
                        containers,
                        views,
                    )?;
                    Contents::Container(new_id)
                }
            };
            if original.active_tab == Some(child) {
                container.active_tab = Some(copied);
            }
            container.contents.push(copied);
        }
        containers.push(container);
        Some(())
    }

    fn containers_for_duplication(&self) -> BTreeMap<ContainerId, ContainerBlueprint> {
        let contents_from_tile_id = self.contents_from_tile_id();
        self.tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| {
                let egui_tiles::Tile::Container(container) = tile else {
                    return None;
                };
                let Contents::Container(id) = *contents_from_tile_id.get(tile_id)? else {
                    return None; // Temporary single-view tabs map directly to their view.
                };
                // Do not silently omit a missing child while reconstructing the source.
                if !container.children().all(|child| {
                    self.tree.tiles.get(*child).is_some()
                        && contents_from_tile_id.contains_key(child)
                }) {
                    return None;
                }
                let mut snapshot = ContainerBlueprint::from_egui_tiles_container(
                    id,
                    container,
                    self.tree.is_visible(*tile_id),
                    &contents_from_tile_id,
                );
                snapshot.display_name = self
                    .container(&id)
                    .and_then(|original| original.display_name.clone());
                Some((id, snapshot))
            })
            .collect()
    }

    pub(super) fn apply_duplicate_container(
        &mut self,
        ctx: &ViewerContext<'_>,
        source: ContainerId,
        target: ContainerId,
    ) {
        let source_tile = Contents::Container(source).as_tile_id();
        if self.tree.tiles.get(source_tile).is_none() {
            return;
        }

        // Construct the whole copy before making any changes, so a missing child cannot leave
        // a partly inserted hierarchy behind.
        let mut containers = Vec::new();
        let mut views = Vec::new();
        let current_containers = self.containers_for_duplication();
        if self
            .duplicate_container_tree(
                &current_containers,
                source,
                target,
                ctx,
                &mut containers,
                &mut views,
            )
            .is_none()
        {
            re_log::warn_once!("Cannot clone a container with missing contents");
            return;
        }

        for view in views {
            let tile_id = Contents::View(view.id).as_tile_id();
            self.tree
                .tiles
                .insert(tile_id, egui_tiles::Tile::Pane(view.id));
            self.tree.set_visible(tile_id, view.visible);
            view.save_to_blueprint_store(ctx);
            self.views.insert(view.id, view);
        }
        for container in containers {
            let tile_id = Contents::Container(container.id).as_tile_id();
            self.tree.tiles.insert(tile_id, container.to_tile());
            self.tree.set_visible(tile_id, container.visible);
            container.save_to_blueprint_store(ctx);
            self.containers.insert(container.id, container);
        }

        let target_tile = Contents::Container(target).as_tile_id();
        if let Some(parent) = self.tree.tiles.parent_of(source_tile) {
            if let Some(egui_tiles::Tile::Container(container)) = self.tree.tiles.get_mut(parent) {
                let position = container
                    .children()
                    .position(|id| *id == source_tile)
                    .unwrap_or(0);
                container.add_child(target_tile);
                self.tree
                    .move_tile_to_container(target_tile, parent, position + 1, true);
            }
        } else {
            self.tree.root = Some(
                self.tree
                    .tiles
                    .insert_tab_tile(vec![source_tile, target_tile]),
            );
        }

        // Reveal the copy when its parent is a tab container.
        self.tree.make_active(|id, _| id == target_tile);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fmt::Write as _};

    use egui_tiles::ContainerKind;
    use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
    use re_sdk_types::components::Visible;
    use re_test_context::TestContext;
    use re_viewer_context::{
        BlueprintContext as _, ContainerId, Contents, ViewerContext, VisitorControlFlow,
    };

    use crate::{
        ContainerBlueprint, ViewBlueprint, ViewContents, ViewportBlueprint, ViewportCommand,
    };

    fn edit<R>(
        test: &TestContext,
        edit: impl FnOnce(&ViewerContext<'_>, ViewportBlueprint) -> R,
    ) -> R {
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
                container_kind: ContainerKind::Horizontal,
                display_name: Some("Uneven row".into()),
                contents: views.drain(..2).collect(),
                col_shares: vec![3.0, 1.0],
                ..Default::default()
            };

            let mut tabs = ContainerBlueprint::default();
            tabs.container_kind = ContainerKind::Tabs;
            tabs.display_name = Some("Notes".into());
            tabs.contents = views;
            tabs.active_tab = Some(tabs.contents[1]);

            let empty = ContainerBlueprint {
                container_kind: ContainerKind::Vertical,
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
            root.container_kind = ContainerKind::Tabs;
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

    fn hierarchy(blueprint: &ViewportBlueprint, id: ContainerId) -> String {
        let mut output = String::new();
        let traversal = blueprint.visit_contents_in_container::<()>(&id, &mut |contents, parents| {
            let indent = "  ".repeat(parents.len());
            match *contents {
                Contents::Container(id) => {
                    let container = blueprint.container(&id).unwrap();
                    let active = container.active_tab.and_then(|active| {
                        container.contents.iter().position(|child| *child == active)
                    });
                    writeln!(output,
                        "{indent}{:?} {:?} visible={} columns={:?} col_shares={:?} row_shares={:?} active={active:?}",
                        container.container_kind, container.display_name, container.visible,
                        container.grid_columns, container.col_shares, container.row_shares,
                    ).unwrap();
                }
                Contents::View(id) => {
                    let view = blueprint.view(&id).unwrap();
                    writeln!(output,
                        "{indent}View {:?} visible={} titlebar={} origin={}",
                        view.display_name, view.visible, view.titlebar, view.space_origin,
                    ).unwrap();
                }
            }
            VisitorControlFlow::Continue
        });
        assert!(traversal.is_continue());
        output
    }

    fn first_view(
        blueprint: &ViewportBlueprint,
        container: ContainerId,
    ) -> re_viewer_context::ViewId {
        blueprint
            .find_contents_in_container_by(&|child| matches!(child, Contents::View(_)), &container)
            .unwrap()
            .as_view_id()
            .unwrap()
    }

    fn assert_independent_copy(
        ctx: &ViewerContext<'_>,
        blueprint: &ViewportBlueprint,
        original: ContainerId,
        copied: ContainerId,
    ) {
        assert_eq!(hierarchy(blueprint, original), hierarchy(blueprint, copied));
        let descendants = |id| {
            let mut contents = Vec::new();
            let traversal = blueprint.visit_contents_in_container::<()>(&id, &mut |child, _| {
                contents.push(*child);
                VisitorControlFlow::Continue
            });
            assert!(traversal.is_continue());
            contents
        };
        let original = descendants(original);
        let copied = descendants(copied);
        let original_ids: HashSet<_> = original.iter().map(Contents::as_entity_path).collect();
        let copied_ids: HashSet<_> = copied.iter().map(Contents::as_entity_path).collect();
        assert!(original_ids.is_disjoint(&copied_ids));
        assert_eq!(copied_ids.len(), copied.len());
        for (original, copied) in std::iter::zip(original, copied) {
            if let (Contents::View(original), Contents::View(copied)) = (original, copied) {
                let original = blueprint.view(&original).unwrap();
                let copied = blueprint.view(&copied).unwrap();
                assert_eq!(original.class_identifier(), copied.class_identifier());
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
        }
    }

    #[test]
    fn clone_nested_and_root_containers() {
        for clone_root in [false, true] {
            let (test, dashboard) = fixture();
            let (source, target) = edit(&test, |ctx, blueprint| {
                let source = if clone_root {
                    blueprint.root_container
                } else {
                    dashboard
                };
                let target = blueprint.duplicate_container(&source, ctx).unwrap();
                blueprint.save_to_blueprint_store(ctx);
                (source, target)
            });
            let (original_view, copied_view) = edit(&test, |ctx, blueprint| {
                let root = blueprint.container(&blueprint.root_container).unwrap();
                assert_ne!(root.id, source);
                assert_ne!(root.id, target);
                assert_eq!(root.container_kind, ContainerKind::Tabs);
                assert_eq!(
                    root.contents,
                    [Contents::Container(source), Contents::Container(target)]
                );
                assert_eq!(root.active_tab, Some(Contents::Container(target)));
                assert_eq!(blueprint.views.len(), 8);
                assert_independent_copy(ctx, &blueprint, source, target);
                if !clone_root {
                    insta::assert_snapshot!("cloned_dashboard", hierarchy(&blueprint, target));
                }
                let copied = blueprint.container(&target).unwrap();
                copied.set_display_name(ctx, Some("Independent copy".into()));
                copied.set_visible(ctx, false);
                let original_view = first_view(&blueprint, source);
                let copied_view = first_view(&blueprint, target);
                assert_eq!(view_visible_override(ctx, copied_view), Some(false));
                ctx.save_blueprint_archetype(
                    ViewContents::base_override_path_for_entity(copied_view, &"robot".into()),
                    &blueprint_archetypes::EntityBehavior::new().with_visible(true),
                );
                (original_view, copied_view)
            });
            edit(&test, |ctx, blueprint| {
                let original = blueprint.container(&source).unwrap();
                let copied = blueprint.container(&target).unwrap();
                assert_ne!(original.display_name.as_deref(), Some("Independent copy"));
                assert!(original.visible);
                assert_eq!(copied.display_name.as_deref(), Some("Independent copy"));
                assert!(!copied.visible);
                assert_eq!(view_visible_override(ctx, original_view), Some(false));
                assert_eq!(view_visible_override(ctx, copied_view), Some(true));
            });
        }
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
            // Fail after earlier children have already been copied.
            let view = blueprint
                .find_contents_in_container_by(
                    &|child| {
                        child.as_view_id().is_some_and(|id| {
                            blueprint.view(&id).unwrap().display_name.as_deref() == Some("Plot 3")
                        })
                    },
                    &source,
                )
                .unwrap()
                .as_view_id()
                .unwrap();
            blueprint.views.remove(&view);
            let old_views = blueprint.views.len();
            let old_containers = blueprint.containers.len();
            blueprint.apply_duplicate_container(ctx, source, ContainerId::random());
            assert_eq!(blueprint.views.len(), old_views);
            assert_eq!(blueprint.containers.len(), old_containers);
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
    fn clone_captures_preceding_layout_changes_only() {
        let (test, source) = fixture();
        let (target, original_tabs) = edit(&test, |ctx, blueprint| {
            let children = &blueprint.container(&source).unwrap().contents;
            let row = children[0].as_container_id().unwrap();
            let tabs = children[1].as_container_id().unwrap();
            let first_tab = blueprint.container(&tabs).unwrap().contents[0]
                .as_view_id()
                .unwrap();
            blueprint.set_container_kind(source, ContainerKind::Vertical);
            blueprint.make_all_children_same_size(&row);
            blueprint.focus_tab(first_tab);
            let target = blueprint.duplicate_container(&source, ctx).unwrap();
            blueprint.set_container_kind(source, ContainerKind::Horizontal);
            blueprint.focus_tab(
                blueprint.container(&tabs).unwrap().contents[1]
                    .as_view_id()
                    .unwrap(),
            );
            blueprint.save_to_blueprint_store(ctx);
            (target, tabs)
        });
        edit(&test, |_, blueprint| {
            assert_eq!(
                blueprint.container(&source).unwrap().container_kind,
                ContainerKind::Horizontal
            );
            let copied = blueprint.container(&target).unwrap();
            assert_eq!(copied.container_kind, ContainerKind::Vertical);
            let row = blueprint
                .container(&copied.contents[0].as_container_id().unwrap())
                .unwrap();
            assert_eq!(row.col_shares, [1.0, 1.0]);
            let tabs = blueprint
                .container(&copied.contents[1].as_container_id().unwrap())
                .unwrap();
            assert_eq!(tabs.active_tab, Some(tabs.contents[0]));
            let original_tabs = blueprint.container(&original_tabs).unwrap();
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
            blueprint.move_contents_to_new_container(moved, ContainerKind::Vertical, source, 1);
            let target = blueprint.duplicate_container(&source, ctx).unwrap();
            blueprint.save_to_blueprint_store(ctx);
            target
        });
        edit(&test, |ctx, blueprint| {
            assert_independent_copy(ctx, &blueprint, source, target);
            let copied = blueprint.container(&target).unwrap();
            let new_view = blueprint
                .view(&copied.contents[0].as_view_id().unwrap())
                .unwrap();
            assert_eq!(new_view.display_name.as_deref(), Some("New view"));
            let nested = blueprint
                .container(&copied.contents[1].as_container_id().unwrap())
                .unwrap();
            assert_eq!(nested.container_kind, ContainerKind::Vertical);
            assert_eq!(nested.contents.len(), 2);
        });
    }

    #[test]
    fn clone_inserts_after_source_in_each_parent_layout() {
        for kind in ContainerKind::ALL {
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
            edit(&test, |_, blueprint| {
                let root = blueprint.container(&blueprint.root_container).unwrap();
                assert_eq!(
                    root.contents,
                    [
                        siblings[0],
                        Contents::Container(source),
                        Contents::Container(target),
                        siblings[1]
                    ]
                );
                assert_eq!(root.container_kind, kind);
                match kind {
                    ContainerKind::Grid => {
                        assert_eq!(root.grid_columns, Some(3));
                        assert_eq!(root.col_shares, [2.0, 3.0, 5.0]);
                        assert_eq!(root.row_shares, [2.0, 3.0, 5.0]);
                    }
                    ContainerKind::Horizontal => assert_eq!(root.col_shares, [2.0, 3.0, 1.0, 5.0]),
                    ContainerKind::Tabs => {
                        assert_eq!(root.active_tab, Some(Contents::Container(target)));
                    }
                    ContainerKind::Vertical => assert_eq!(root.row_shares, [2.0, 3.0, 1.0, 5.0]),
                }
            });
        }
    }

    fn view_visible_override(
        ctx: &ViewerContext<'_>,
        view: re_viewer_context::ViewId,
    ) -> Option<bool> {
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
            assert_independent_copy(ctx, &blueprint, source, target);
        });
    }
}
