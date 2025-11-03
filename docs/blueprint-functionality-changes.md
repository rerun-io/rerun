# Blueprint functionality changes

This document lists changes to the Rerun blueprint system's functionality or behavior that are reflected in the documentation updates. These are distinct from pure documentation improvements (see `blueprint-documentation-improvements.md`).

## Summary

After detailed analysis of the git diff across all modified files, **no definitive evidence of blueprint functionality changes** was found. The documentation changes appear to be primarily clarifications, reorganizations, and more explicit explanations of existing behavior rather than documenting new features or changed behavior.

## Potential changes requiring verification

The following items show changes in how blueprints are described that *could* indicate functionality changes, but more likely represent clarifications of existing behavior. These should be verified against the actual codebase changes:

### 1. Panel visibility control

**Location:** `content/concepts/blueprint.md`, line 21-24

**Old Documentation:**
```markdown
Every aspect of what the Viewer displays is controlled by the blueprint. This
includes the type and content of the different views, the organization and
layout of the different containers, and the configuration and styling properties
of the individual data visualizers
```

**New Documentation:**
```markdown
Blueprints give you complete control over the Viewer's layout and configuration:

-   **Panel visibility**: Whether panels like the blueprint panel, selection panel, and time panel are expanded or collapsed
-   **Layout structure**: How views are arranged using containers (Grid, Horizontal, Vertical, Tabs)
-   **View types and configuration**: What kind of views display your data (2D/3D spatial, maps, charts, text logs, etc.) and their specific settings
-   **Visual properties**: Styling like backgrounds, colors, zoom levels, time ranges, and visual bounds
```

**Analysis:**
The new documentation explicitly lists "Panel visibility" (whether panels are expanded or collapsed) as something blueprints control. The old documentation said "every aspect" but didn't specifically call out panel visibility.

**Conclusion:** This is most likely a **clarification** of existing functionality rather than a new feature. Panel visibility was likely always controlled by blueprints, but now it's explicitly documented.

**Verification needed:** Check if panel expansion/collapse state storage in blueprints is a new feature or was always present.

---

### 2. Complete blueprint state coverage

**Location:** `content/concepts/blueprint.md`, line 44-47

**Old Documentation:**
```markdown
In general, if you can modify an aspect of how something looks through the
viewer, you are actually modifying the blueprint. (Note that while there may be
some exceptions to this rule at the moment, the intent is to eventually migrate
all state to the blueprint.)
```

**New Documentation:**
```markdown
In general, if you can modify an aspect of how something looks through the Viewer, you are actually modifying the blueprint.
```

**Analysis:**
The old documentation contained a caveat: "(Note that while there may be some exceptions to this rule at the moment, the intent is to eventually migrate all state to the blueprint.)" This caveat has been removed in the new documentation.

**Conclusion:** This could indicate that:
- **Option A**: All viewer state has now been migrated to blueprints (functionality change)
- **Option B**: The caveat was removed for simplicity/clarity even though minor exceptions still exist (documentation improvement)

**Verification needed:** Check if there are any remaining viewer state items that are NOT stored in blueprints. If not, this represents completion of a migration effort.

---

### 3. Application ID blueprint binding behavior

**Location:** `content/concepts/blueprint.md`, line 28-39

**Old Documentation:**
```markdown
## Loose coupling

The blueprint and the recording are only loosely coupled. Rerun uses the
[application ID](apps-and-recordings.md) to determine whether a blueprint and a
recording should be used together, but they are not directly linked beyond that.

This means that either can be changed independently of the other. Keeping the
blueprint constant while changing the recording will allow you to compare
different datasets using a consistent set of views. On the other hand, changing
the blueprint while keeping a recording constant will allow you to view the same
data in different ways.
```

**New Documentation:**
```markdown
## Application IDs: Binding Blueprints to Data

The [Application ID](apps-and-recordings.md) is how blueprints connect to your data. This is a critical concept:

**All recordings that share the same Application ID will use the same blueprint.**

This loose coupling between blueprints and recordings means:
-   You can keep the blueprint constant while changing the recording to compare different datasets with consistent views
-   You can change the blueprint while keeping a recording constant to view the same data in different ways
-   When you save blueprint changes in the Viewer, those changes apply to all recordings with that Application ID

Think of the Application ID as the "key" that binds a blueprint to a specific type of recording. If you want recordings to share the same layout, give them the same Application ID.
```

**Analysis:**
The new documentation adds: "When you save blueprint changes in the Viewer, those changes apply to all recordings with that Application ID."

This statement makes explicit the persistence behavior of blueprint changes across recordings with the same Application ID. The old documentation didn't explicitly mention this persistence behavior.

**Conclusion:** This is most likely a **clarification** of existing behavior. The persistence mechanism was likely always present (blueprints are stored per Application ID), but the old documentation didn't explicitly explain this to users.

**Verification needed:** Confirm that blueprint persistence by Application ID was always the behavior, not a new feature.

---

### 4. Reset behavior conceptual model

**Location:** `content/concepts/blueprint.md`, line 41-56

**Old Documentation:**
```markdown
## Current, default, and heuristics blueprints

Blueprints may originate from multiple sources.

- The "current blueprint" for a given application ID is the one that is used by the Viewer to display data at any given time. It is updated for each change made to the visualization within the viewer, and may be saved to a blueprint file at any time.
- The "default blueprint" is a snapshot that is set or updated when a blueprint is received from code or loaded from a file. The current blueprint may be reset to default blueprint at any time by using the "reset" button in the blueprint panel's header.
- The "heuristic blueprint" is an automatically-produced blueprint based on the recording data. When no default blueprint is available, the heuristic blueprint is used when resetting the current blueprint. It is also possible to reset to the heuristic blueprint in the selection panel after selecting an application.
```

**New Documentation:**
```markdown
## Reset Behavior: Heuristic vs Default

The Viewer provides two types of blueprint reset, accessible from the blueprint panel:

### Reset to Heuristic Blueprint
This generates a new blueprint automatically based on your current data. The Viewer analyzes what you've logged and creates an appropriate layout using built-in heuristics. This is useful when you want to start fresh and let Rerun figure out a reasonable layout.

### Reset to Default Blueprint
This returns to your programmatically specified blueprint (sent from code) or a saved blueprint file (`.rbl`). If you've sent a blueprint using `rr.send_blueprint()` or loaded a `.rbl` file, this becomes your "default." The reset button in the blueprint panel will restore this default whenever you need it.

When no default blueprint has been set, the reset button will use the heuristic blueprint instead.
```

**Analysis:**
The conceptual model has changed from describing "three types of blueprints" (current, default, heuristic) to describing "two types of reset behavior" (reset to heuristic, reset to default). The "current blueprint" concept is no longer explicitly mentioned.

**Conclusion:** This is a **conceptual reorganization** rather than a functional change. The underlying behavior appears the same:
- There's still a blueprint being used at any given time (formerly "current blueprint", now just "the blueprint")
- You can still reset to a default blueprint (sent from code or loaded from file)
- You can still reset to an automatically-generated heuristic blueprint

The change simplifies the mental model by focusing on user actions (reset behaviors) rather than three separate blueprint types.

**Verification needed:** Confirm that the reset behaviors work the same way as before, just explained differently.

---

## Changes NOT found

The following were explicitly checked and found **NO evidence** of changes:

1. **No new blueprint API methods mentioned**: The documentation references existing methods like `rr.send_blueprint()` and `default_blueprint` parameter without indicating these are new.

2. **No new view types**: The view types mentioned (2D/3D spatial, maps, charts, text logs) appear to be examples of existing functionality.

3. **No new container types**: The four container types (Horizontal, Vertical, Grid, Tabs) are mentioned in both old and new documentation.

4. **No new visualizer capabilities**: The discussion of visualizers and overrides references existing functionality.

5. **No new blueprint file format**: Blueprint files (`.rbl`) are mentioned in both old and new documentation without indicating format changes.

6. **No new reset mechanisms**: The reset button functionality is described similarly in both versions.

7. **No changes to blueprint architecture**: The technical description of blueprints as ECS data with a blueprint timeline remains the same.

## Methodology

This analysis was conducted by:
1. Examining git diffs for all modified files using `git diff HEAD -- <file>`
2. Reading both old and new versions of modified files in full
3. Comparing section-by-section to identify semantic changes vs stylistic changes
4. Reading deleted files to understand removed content
5. Checking all cross-reference updates for hints about functionality changes

## Conclusion

The documentation changes appear to be a **major documentation improvement effort** with no clear evidence of underlying blueprint functionality changes. The changes focus on:
- Making implicit behaviors explicit
- Reorganizing content for better user comprehension
- Providing more detailed examples and use cases
- Consolidating scattered documentation into cohesive guides

Any actual blueprint functionality changes would need to be confirmed by examining the source code commits, not just the documentation updates.
