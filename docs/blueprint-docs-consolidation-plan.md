# Blueprint documentation consolidation & fix plan

## Overview
This plan consolidates blueprint documentation into fewer, more focused pages while fixing outdated content, missing hyperlinks, and inconsistent type documentation.

**Goal**: Minimize the number of places blueprint information appears while maintaining clear organization.

## Core blueprint concepts (Authoritative source)

Based on the definitive blueprint content, here are the key concepts to communicate:

**Formula**: `Data + Blueprint = Rerun Viewer`

**What Blueprints Control**:
- Panel visibility (blueprint panel, selection panel, time panel)
- Layout structure (containers: grid, horizontal, vertical, tabs)
- View types and their configuration (2D/3D spatial, maps, charts, logs, etc.)
- Visual properties (backgrounds, zoom levels, time ranges, visual bounds)

**Application ID Concept** (Critical):
- All applications sharing the same Application ID share the same blueprint
- This is how blueprints "bind" to recordings
- Use same app ID for same layout across sessions

**Two Types of Reset**:
1. **Reset to Heuristic Blueprint**: Auto-generated based on your data
2. **Reset to Default Blueprint**: Returns to programmatically specified blueprint (or saved `.rbl` file)

**Three Ways to Work with Blueprints**:
1. **Interactively**: Drag-and-drop, split containers, rearrange views in the UI
2. **Save/Load Files**: Save `.rbl` files, load them back, share across projects
3. **Programmatically**: Write blueprint code that configures the viewer for specific scenarios

---

## Proposed documentation structure (Post-Consolidation)

### Target: 4 main blueprint documentation pages

1. **`concepts/blueprint.md`** - Complete conceptual guide
   - What blueprints are (Data + Blueprint = Viewer)
   - Application ID binding concept
   - What blueprints control (panels, layout, views, visual properties)
   - Reset types (heuristic vs default)
   - The three ways to work with blueprints (interactive, file-based, programmatic)
   - When to use each approach
   - Examples of common use cases

2. **`reference/viewer/blueprint.md`** - Blueprint Panel UI reference
   - UI controls and buttons
   - How to use the blueprint panel
   - Screenshot of current UI
   - Brief reference only (links to concepts for details)

3. **`getting-started/configure-the-viewer.md`** - Unified practical guide
   - **Merge three existing pages into one comprehensive tutorial**
   - Section 1: Interactive configuration (drag-and-drop, containers, splitting)
   - Section 2: Saving and loading `.rbl` files
   - Section 3: Programmatic blueprints (code tutorial)
   - Progressive examples building on each other

4. **Python API Documentation** - Auto-generated reference
   - Keep as technical reference
   - Fix all linking and type issues
   - Add examples linking back to getting-started guide

### Pages to Eliminate/Merge

**Eliminate these as separate pages**:
- ❌ `getting-started/configure-the-viewer/interactively.md` → merge into unified getting-started
- ❌ `getting-started/configure-the-viewer/save-and-load.md` → merge into unified getting-started
- ❌ `getting-started/configure-the-viewer/through-code-tutorial.md` → merge into unified getting-started
- ❌ `howto/visualization/configure-viewer-through-code.md` → delete or merge into unified getting-started
- ❌ `howto/visualization/reuse-blueprints.md` → merge relevant content into unified getting-started

**Keep but update**:
- ✅ `concepts/blueprint.md` - make this THE comprehensive conceptual guide
- ✅ `reference/viewer/blueprint.md` - minimal UI reference
- ✅ `getting-started/configure-the-viewer.md` - unified practical tutorial
- ✅ Python API docs - fix technical issues
- ✅ Individual view type docs - keep as reference but ensure consistency

---

## Phase 1: content consolidation

### 1.1 create comprehensive `concepts/blueprint.md`

**Source Content**:
- Extract best content from existing `concepts/blueprint.md`
- Pull practical examples from getting-started guides
- Incorporate use cases from howto guides

**New Structure**:
```markdown
# Blueprints

## What are Blueprints?
- Formula: Data + Blueprint = Rerun Viewer
- Blueprints as viewer configuration layer

## Application IDs: Binding Blueprints to Data
- How app IDs work
- Why same app ID = same blueprint
- When to use different app IDs

## What Blueprints Control
- Panel visibility
- Container layouts (grid, horizontal, vertical, tabs)
- View types and configuration
- Visual properties

## Reset Behavior
- Reset to heuristic (auto-generated)
- Reset to default (programmatically specified or saved)
- When each is useful

## Three Ways to Work with Blueprints
1. Interactive (manual UI editing)
2. File-based (.rbl files)
3. Programmatic (code)

## Common Use Cases
- Debugging specific scenarios (e.g., robot diagnostics)
- Sharing layouts with team
- Templating for different data types
- Dynamic viewer configuration
```

**Actions**:
- [ ] Write comprehensive `concepts/blueprint.md` using authoritative content
- [ ] Include all key concepts without redundancy
- [ ] Add cross-references to getting-started and reference docs
- [ ] Keep focused and concise (aim for single-page comprehension)

### 1.2 create unified `getting-started/configure-the-viewer.md`

**Merge these three pages**:
1. `interactively.md`
2. `save-and-load.md`
3. `through-code-tutorial.md`

**New Structure**:
```markdown
# Configure the Viewer with Blueprints

Brief intro linking to concepts/blueprint.md for theory.

## Interactive Configuration
- Drag and drop views
- Creating containers (horizontal, vertical, grid, tabs)
- Splitting views
- Rearranging layout
- [Content from interactively.md]

## Saving and Loading Blueprints
- Saving .rbl files from UI
- Loading .rbl files
- Command-line blueprint loading
- Sharing blueprints across projects
- [Content from save-and-load.md]

## Programmatic Blueprints
- Complete code tutorial (stock market example or similar)
- Creating views
- Arranging containers
- Sending blueprints
- [Content from through-code-tutorial.md]

## Practical Examples
- Example 1: Text logs only
- Example 2: 2D scene with custom background
- Example 3: Multi-view grid layout
- Example 4: Tabs organization
```

**Actions**:
- [ ] Merge three getting-started pages into one
- [ ] Ensure progressive tutorial structure
- [ ] Remove redundancy between sections
- [ ] Delete old separate pages after merge
- [ ] Update navigation/sidebar configuration

### 1.3 delete or merge howto guides

**`howto/visualization/configure-viewer-through-code.md`**:
- [ ] Review content vs unified getting-started guide
- [ ] Move any unique API examples to unified guide
- [ ] Delete this page (redirect to getting-started)

**`howto/visualization/reuse-blueprints.md`**:
- [ ] Extract any unique reuse patterns
- [ ] Merge into unified getting-started guide (save/load section)
- [ ] Delete this page (redirect to getting-started)

**`howto/visualization/fixed-window-plot.md`**:
- [ ] Review if blueprint content should move
- [ ] Update to reference unified getting-started guide
- [ ] Keep if it's a specific use case

**`howto/visualization/geospatial-data.md`**:
- [ ] Review if blueprint content should move
- [ ] Update to reference unified getting-started guide
- [ ] Keep if it's a specific use case

---

## Phase 2: reference documentation updates

### 2.1 update `reference/viewer/blueprint.md`

**Make this a minimal UI reference only**:
- What the blueprint panel shows
- UI controls (reset to heuristic, reset to default, save, load)
- Clear explanation of reset types
- Current screenshot matching UI
- Link to `concepts/blueprint.md` for details

**Actions**:
- [ ] Take new screenshot of blueprint panel in current Rerun version
- [ ] Rewrite to focus purely on UI reference
- [ ] Add clear definitions:
  - "Reset to Heuristic Blueprint": Auto-generated layout based on your data
  - "Reset to Default Blueprint": Returns to programmatically specified or saved blueprint
- [ ] Remove conceptual explanations (link to concepts instead)
- [ ] Keep it brief (should be a quick reference)

### 2.2 update `reference/viewer/selection.md`

**Issues to fix**:
- Visibility checkbox location changed
- Selection history behavior unclear

**Actions**:
- [ ] Verify current UI for visibility controls
- [ ] Update screenshot if needed
- [ ] Test selection history feature
- [ ] Update or remove selection history documentation
- [ ] Ensure consistency with blueprint.md

### 2.3 update view type reference pages

**All 11 view pages in `reference/types/views/`**:
- [ ] Ensure consistent blueprint section structure
- [ ] Link to unified getting-started guide
- [ ] Remove redundant conceptual explanations
- [ ] Keep focused on view-specific configuration

---

## Phase 3: Python API documentation fixes

**Strategy**: Fix technical issues while maintaining API docs as pure technical reference. Add links back to unified getting-started guide for examples.

### 3.1 fix type linking system

**Core Issue**: `separate_signature` affects hyperlinks, and many types are not documented/exposed

**Actions**:
- [ ] Investigate `separate_signature` configuration for proper hyperlinking
- [ ] Fix class method signatures to work with hyperlinking enabled
- [ ] Create/expose documentation for all `*Like` type aliases:
  - `BlueprintLike`, `EntityPathLike`, `PanelStateLike`
  - `Rgba32Like`, `BackgroundKind`, `ContainerKindLike`
  - `EntityPathArrayLike`, `Float32ArrayLike`, `BoolLike`, `UInt32Like`, `Utf8Like`
- [ ] Create/expose documentation for utility types:
  - `TopPanel`, `DescribedComponentBatch`, `BackgroundExt`, `Archetype`
- [ ] Decision: Create dedicated "Types and Aliases" reference page OR document inline
- [ ] Ensure all type references hyperlink correctly

### 3.2 fix blueprint APIs class documentation

**Blueprint Class**:
- [ ] Fix `BlueprintPart` docstring (complete list or remove enumeration)
- [ ] Change `app_id` → `application_id` for consistency with authoritative terminology
- [ ] Complete `make_activate` sentence fragment
- [ ] Add link to getting-started guide for examples

**Container Classes** (Horizontal, Vertical, Grid, Tabs):
- [ ] Rewrite Container base class description (clarify "ergonomic helpers" language)
- [ ] Clarify Grid layout behavior:
  - Grids are NOT always square
  - `grid_columns` determines column count, rows auto-expand
  - More flexible than Horizontal/Vertical
- [ ] Improve `column_shares` and `row_shares` descriptions (be more concise)
- [ ] Clarify constructor iterator parameter patterns
- [ ] Add Tabs class description:
  - How tabs organize views
  - Behavior when tabs are hidden (other content expands)
- [ ] Decide on stability marking policy (apply consistently or remove)

**Views Class**:
- [ ] Remove enumeration of subclasses from Views docstring (or auto-generate complete list)
- [ ] Clarify organization (top-level vs Views category)
- [ ] Add examples linking to getting-started guide

**BlueprintPanel Class**:
- [ ] Document `PanelStateLike` type
- [ ] Verify panel state options

### 3.3 fix blueprint archetypes documentation

**EntityBehavior**:
- [ ] Resolve or remove TODO from docstring
- [ ] Complete documentation

**ViewContents**:
- [ ] Fix `QueryExpressions` hyperlinking
- [ ] Align argument type with actual parameter type
- [ ] Link to entity query expression documentation

**All Archetypes**:
- [ ] Ensure all referenced types are documented
- [ ] Verify consistency across archetype docs
- [ ] Add links back to getting-started for usage examples

### 3.4 fix blueprint components documentation

**Enum Sorting**:
- [ ] Change enum sorting from alphabetical to value-based order
- [ ] Apply consistently across all component enums

**Type Linking**:
- [ ] Fix all component type hyperlinks
- [ ] Ensure consistency with other API documentation

---

## Phase 4: Cross-Cutting standards & cleanup

### 4.1 establish consistent terminology

**Reset Terminology** (Critical):
- [ ] Document standard terms:
  - "Reset to Heuristic Blueprint" = auto-generated based on data
  - "Reset to Default Blueprint" = return to saved/programmatic blueprint
- [ ] Apply consistently in: concepts, reference, getting-started, Python docs
- [ ] Add to glossary if one exists

**Application ID Terminology**:
- [ ] Consistently use "Application ID" (not "app ID", "app_id", etc.)
- [ ] Apply across all documentation
- [ ] Emphasize in concepts page

**Container Terminology**:
- [ ] Establish consistent names: Grid, Horizontal, Vertical, Tabs
- [ ] Use "container" consistently (not "layout", "panel", etc. when referring to containers)

### 4.2 fix entity query expression references

**Actions**:
- [ ] Create anchor link target in entity query expression docs
- [ ] Update all references to link to specific section (not just page)
- [ ] Verify completeness of query expression documentation

### 4.3 resolve stale issue links

**Actions**:
- [ ] Identify all GitHub issue links in blueprint documentation
- [ ] Update or remove stale issues
- [ ] Add context for remaining issue links (why they're referenced)

---

## Phase 5: navigation & discoverability

### 5.1 update internal Cross-References

**From concepts/blueprint.md**:
- [ ] Link to getting-started for practical tutorials
- [ ] Link to reference/viewer/blueprint.md for UI details
- [ ] Link to Python API docs for programmatic reference

**From getting-started/configure-the-viewer.md**:
- [ ] Link to concepts/blueprint.md for theory
- [ ] Link to Python API docs for detailed API reference
- [ ] Link to specific view type docs for view configuration

**From reference/viewer/blueprint.md**:
- [ ] Link to concepts/blueprint.md for conceptual understanding
- [ ] Link to getting-started for how-to use the UI

**From Python API docs**:
- [ ] Add links to getting-started guide for practical examples
- [ ] Link to concepts for understanding blueprint architecture

### 5.2 update Navigation/Sidebar configuration

**After consolidation, update sidebar to reflect**:
```
Getting Started
  └─ Configure the Viewer [unified page]

Concepts
  └─ Blueprints [comprehensive]

Reference
  ├─ Viewer
  │   ├─ Blueprint [UI only]
  │   └─ Selection [updated]
  └─ Types
      └─ Views [11 view types]

Python API
  ├─ Blueprint APIs
  ├─ Blueprint Archetypes
  └─ Blueprint Components
```

**Actions**:
- [ ] Remove deleted pages from navigation
- [ ] Update page titles if changed
- [ ] Add redirects from old URLs to new consolidated pages

### 5.3 create redirect rules

**For deleted pages, create redirects**:
- `getting-started/configure-the-viewer/interactively.md` → `getting-started/configure-the-viewer.md#interactive-configuration`
- `getting-started/configure-the-viewer/save-and-load.md` → `getting-started/configure-the-viewer.md#saving-and-loading-blueprints`
- `getting-started/configure-the-viewer/through-code-tutorial.md` → `getting-started/configure-the-viewer.md#programmatic-blueprints`
- `howto/visualization/configure-viewer-through-code.md` → `getting-started/configure-the-viewer.md#programmatic-blueprints`
- `howto/visualization/reuse-blueprints.md` → `getting-started/configure-the-viewer.md#saving-and-loading-blueprints`

**Actions**:
- [ ] Configure redirect rules in documentation system
- [ ] Test all redirects work correctly
- [ ] Verify anchor links work after redirects

---

## Phase 6: validation & quality assurance

### 6.1 content validation

**Conceptual Accuracy**:
- [ ] Verify all pages reflect authoritative blueprint content
- [ ] Ensure "Data + Blueprint = Viewer" concept appears in concepts page
- [ ] Verify Application ID binding is explained clearly
- [ ] Confirm reset types are correctly documented everywhere

**Technical Accuracy**:
- [ ] Verify all code examples work with current API
- [ ] Test all programmatic examples from getting-started guide
- [ ] Validate all Python API signatures are current

### 6.2 link validation

**Actions**:
- [ ] Run automated link checker on all modified pages
- [ ] Test all internal links (page-to-page)
- [ ] Test all anchor links (within-page and cross-page)
- [ ] Verify external links are not stale
- [ ] Test all redirects

### 6.3 screenshot & media validation

**Actions**:
- [ ] Update screenshot in reference/viewer/blueprint.md
- [ ] Verify screenshots match Rerun 0.25.1+ UI
- [ ] Check all embedded videos/GIFs still relevant
- [ ] Ensure consistent screenshot style guide

### 6.4 code example validation

**Actions**:
- [ ] Run all Python snippets in getting-started guide
- [ ] Test save/load blueprint examples
- [ ] Verify examples in concepts page work
- [ ] Check view type examples in reference docs

### 6.5 completeness check

**Review each consolidated page**:
- [ ] `concepts/blueprint.md` covers all key concepts
- [ ] `getting-started/configure-the-viewer.md` has complete workflow coverage
- [ ] `reference/viewer/blueprint.md` covers all UI elements
- [ ] Python API docs have no broken references

---

## Execution order

### Phase 1: content consolidation (Week 1)
1. Create new comprehensive `concepts/blueprint.md`
2. Merge three pages into unified `getting-started/configure-the-viewer.md`
3. Delete/merge howto guides

### Phase 2: reference updates (Week 1-2)
1. Update `reference/viewer/blueprint.md` with new screenshot and terminology
2. Fix `reference/viewer/selection.md`
3. Light pass on view type reference pages

### Phase 3: Python API fixes (Week 2)
1. Fix type linking system
2. Update class documentation (Blueprint, Container, Views)
3. Fix archetypes and components

### Phase 4: standards & cleanup (Week 2-3)
1. Establish and apply consistent terminology
2. Fix entity query expression references
3. Remove stale issue links

### Phase 5: navigation (Week 3)
1. Update cross-references
2. Update sidebar configuration
3. Create and test redirects

### Phase 6: validation (Week 3)
1. Content validation
2. Link validation
3. Screenshot updates
4. Code example testing
5. Final completeness check

---

## Success criteria

**Consolidation Goals Met**:
- [ ] Blueprint documentation reduced from 8+ pages to 4 main pages
- [ ] No redundant content between pages
- [ ] Clear separation: concepts (why), getting-started (how), reference (what)

**Content Quality**:
- [ ] All pages reflect authoritative blueprint content
- [ ] Reset terminology consistent (heuristic vs default)
- [ ] Application ID concept clearly explained
- [ ] "Data + Blueprint = Viewer" formula appears in concepts

**Technical Correctness**:
- [ ] All Python API types documented and hyperlinked
- [ ] No references to undocumented types
- [ ] Enum types sorted by value
- [ ] All code examples run successfully

**User Experience**:
- [ ] Screenshots match current UI (0.25.1+)
- [ ] All internal links work
- [ ] All redirects work
- [ ] Clear navigation between pages
- [ ] Progressive learning path (concepts → getting-started → reference)

**No Regression**:
- [ ] All old URLs redirect correctly
- [ ] No broken external references
- [ ] No lost content from consolidation

---

## Decision log

### Resolved decisions:
1. **Structure**: 4 main pages (concepts, getting-started, reference, API docs)
2. **Consolidation**: Merge 3 getting-started pages into 1
3. **Howto guides**: Merge or delete (content moved to getting-started)
4. **Reset terminology**: "Heuristic" vs "Default" (from authoritative source)
5. **Application ID**: Consistently use full "Application ID" terminology

### Decisions needed:
1. **Type documentation**: Dedicated types reference page OR inline documentation?
2. **Enum sorting**: Confirm value-based sorting is desired behavior
3. **Stability marking**: Policy for transitivity of unstable markers?
4. **Views organization**: Keep at top-level AND in category, or choose one?
5. **Selection history**: Is this feature still supported? Update or remove docs?

### Open questions:
1. What Rerun version should screenshots reflect? (Assuming 0.25.1+)
2. Should we create a glossary for blueprint terminology?
3. Do we need a migration guide for users of old documentation structure?
