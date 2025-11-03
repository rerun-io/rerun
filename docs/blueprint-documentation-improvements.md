# Blueprint documentation improvements

This document provides a comprehensive, detailed list of all documentation improvements made that do NOT represent changes to blueprint functionality. These are pure documentation enhancements focused on clarity, organization, usability, and completeness.

## Table of contents

1. [File Structure Changes](#file-structure-changes)
2. [content/concepts/blueprint.md Changes](#contentconceptsblueprintmd-changes)
3. [content/getting-started/configure-the-viewer.md Changes](#contentgetting-startedconfigure-the-viewermd-changes)
4. [Cross-Reference Updates](#cross-reference-updates)
5. [Summary Statistics](#summary-statistics)

---

## File structure changes

### Files deleted (Consolidation)

Five separate documentation files were deleted and their content was consolidated into larger, more comprehensive guides:

1. **`content/getting-started/configure-the-viewer/interactively.md`** (180 lines)
   - **Reason**: Content merged into main `configure-the-viewer.md` under "Interactive Configuration" section
   - **Old title**: "Configure the Viewer interactively"
   - **New location**: `configure-the-viewer.md` lines 30-190

2. **`content/getting-started/configure-the-viewer/save-and-load.md`** (25 lines)
   - **Reason**: Content merged into main `configure-the-viewer.md` under "Save and Load Blueprint Files" section
   - **Old title**: "Save and load Viewer configuration files"
   - **New location**: `configure-the-viewer.md` lines 194-226

3. **`content/getting-started/configure-the-viewer/through-code-tutorial.md`** (estimated ~400 lines)
   - **Reason**: Content merged into main `configure-the-viewer.md` under "Programmatic Blueprints" section
   - **Old title**: Likely "Configure the Viewer through code" or similar
   - **New location**: `configure-the-viewer.md` lines 228-598

4. **`content/howto/visualization/configure-viewer-through-code.md`**
   - **Reason**: Consolidated into getting-started guide
   - **Note**: Duplicate/related content to through-code-tutorial.md

5. **`content/howto/visualization/reuse-blueprints.md`**
   - **Reason**: Concepts consolidated into the main blueprint documentation
   - **Note**: Information about reusing blueprints now in concepts/blueprint.md and getting-started guide

### Files modified (Major rewrites)

Two files received substantial rewrites:

1. **`content/concepts/blueprint.md`**
   - **Change**: 109 lines → 153 lines (+44 lines, +40% growth)
   - **Nature**: Complete reorganization and expansion of content

2. **`content/getting-started/configure-the-viewer.md`**
   - **Change**: 23 lines → 598 lines (+575 lines, +2500% growth)
   - **Nature**: Brief overview expanded into comprehensive tutorial

### Files modified (Minor updates)

Eight files received minor updates (primarily link fixes):

1. `content/concepts/apps-and-recordings.md`
2. `content/getting-started/navigating-the-viewer.md`
3. `content/howto/configure-viewer-through-code.md`
4. `content/howto/integrations/embed-notebooks.md`
5. `content/howto/visualization/geospatial-data.md`
6. `content/reference/dataframes.md`
7. `content/reference/viewer/blueprint.md`
8. `content/reference/viewer/viewport.md`

---

## content/concepts/blueprint.md changes

### Section-Level reorganization

#### 1. "blueprints and recordings" → "What are blueprints?"

**Old Section Header** (line 6):
```markdown
## Blueprints and recordings
```

**New Section Header** (line 6):
```markdown
## What are Blueprints?
```

**Changes:**
- **Header**: More question-oriented and user-friendly
- **Content restructuring**: Changed from technical description to simple formula

**Old Introduction** (lines 8-13):
```markdown
<!-- source: Rerun Design System/Documentation schematics -->
<img src="https://static.rerun.io/6e2095a0ffa4f093deb59848b7c294581ded4678_blueprints_and_recordings.png" width="550px">

When you are working with the Rerun viewer, there are two separate pieces that
combine to produce what you see: the "recording" and the "blueprint."
```

**New Introduction** (lines 8-10):
```markdown
When you work with the Rerun Viewer, understanding blueprints is essential. The formula is simple:

**Data + Blueprint = Rerun Viewer**
```

**Improvements:**
- Removed decorative image (simplification)
- Added memorable formula for conceptual clarity
- More direct and engaging opening
- Shorter, punchier sentences

**Old Bullet Points** (lines 14-16):
```markdown
-   The recording provides the actual data you are visualizing.
-   The blueprint is the configuration that determines how the data from the
    recording is displayed.
```

**New Bullet Points** (lines 12-13):
```markdown
-   **The recording** provides the actual data you are visualizing
-   **The blueprint** determines how that data is displayed
```

**Improvements:**
- Added bold formatting for key terms
- Simplified second bullet (removed "configuration" and "from the recording")
- Made parallel structure more consistent

**Old Closing** (lines 18-22):
```markdown
Both of these pieces are crucial—without the recording there is nothing to
show, and without the blueprint there is no way to show it. Even if you have
used Rerun before without explicitly loading a blueprint, the Viewer was
actually creating one for you. Without a blueprint, there is literally nothing
for the Viewer to display.
```

**New Closing** (line 15):
```markdown
Both pieces are crucial. Without a recording there is nothing to show. Without a blueprint there is no way to show it. Even when you use Rerun without explicitly loading a blueprint, the Viewer creates one automatically for you.
```

**Improvements:**
- Broke into shorter sentences for better scannability
- Removed redundant last sentence
- Changed "the Viewer was actually creating" to "the Viewer creates" (present tense, more direct)
- More concise overall

---

#### 2. "loose coupling" → "Application IDs: binding blueprints to data"

**Old Section Header** (line 24):
```markdown
## Loose coupling
```

**New Section Header** (line 28):
```markdown
## Application IDs: Binding Blueprints to Data
```

**Improvements:**
- Header now explicitly names the concept (Application IDs) instead of abstract term
- Action-oriented ("Binding") helps users understand the purpose
- More descriptive and searchable

**Old Introduction** (lines 26-28):
```markdown
The blueprint and the recording are only loosely coupled. Rerun uses the
[application ID](apps-and-recordings.md) to determine whether a blueprint and a
recording should be used together, but they are not directly linked beyond that.
```

**New Introduction** (lines 30-32):
```markdown
The [Application ID](apps-and-recordings.md) is how blueprints connect to your data. This is a critical concept:

**All recordings that share the same Application ID will use the same blueprint.**
```

**Improvements:**
- Emphasized importance ("This is a critical concept")
- Made the key rule highly visible with bold formatting
- More direct and assertive language
- Removed technical jargon like "loosely coupled"

**Old Implications** (lines 30-34):
```markdown
This means that either can be changed independently of the other. Keeping the
blueprint constant while changing the recording will allow you to compare
different datasets using a consistent set of views. On the other hand, changing
the blueprint while keeping a recording constant will allow you to view the same
data in different ways.
```

**New Implications** (lines 34-39):
```markdown
This loose coupling between blueprints and recordings means:
-   You can keep the blueprint constant while changing the recording to compare different datasets with consistent views
-   You can change the blueprint while keeping a recording constant to view the same data in different ways
-   When you save blueprint changes in the Viewer, those changes apply to all recordings with that Application ID

Think of the Application ID as the "key" that binds a blueprint to a specific type of recording. If you want recordings to share the same layout, give them the same Application ID.
```

**Improvements:**
- Converted prose into bulleted list for scannability
- Added third bullet point making persistence explicit
- Added memorable metaphor ("key" that binds)
- Added practical guidance ("If you want recordings to share…")
- Retained "loose coupling" term but only as transitional phrase

---

#### 3. "what the blueprint controls" → "What blueprints control"

**Old Section Header** (line 36):
```markdown
## What the blueprint controls
```

**New Section Header** (line 17):
```markdown
## What Blueprints Control
```

**Improvements:**
- Capitalized "Blueprints" for consistency with other headers
- This section was moved earlier in the document (better information architecture)

**Old Content** (lines 38-42):
```markdown
Every aspect of what the Viewer displays is controlled by the blueprint. This
includes the type and content of the different views, the organization and
layout of the different containers, and the configuration and styling properties
of the individual data visualizers (see [Visualizers and Overrides](visualizers-and-overrides.md)
for more details).
```

**New Introduction** (line 19):
```markdown
Blueprints give you complete control over the Viewer's layout and configuration:
```

**New Bulleted List** (lines 21-24):
```markdown
-   **Panel visibility**: Whether panels like the blueprint panel, selection panel, and time panel are expanded or collapsed
-   **Layout structure**: How views are arranged using containers (Grid, Horizontal, Vertical, Tabs)
-   **View types and configuration**: What kind of views display your data (2D/3D spatial, maps, charts, text logs, etc.) and their specific settings
-   **Visual properties**: Styling like backgrounds, colors, zoom levels, time ranges, and visual bounds
```

**Improvements:**
- Converted prose into structured bullet list
- Made specific examples explicit (panel types, container types, view types)
- Added concrete examples in parentheses
- Easier to scan and understand scope
- Removed link to Visualizers (simplified)

**Old Closing** (lines 44-47):
```markdown
In general, if you can modify an aspect of how something looks through the
viewer, you are actually modifying the blueprint. (Note that while there may be
some exceptions to this rule at the moment, the intent is to eventually migrate
all state to the blueprint.)
```

**New Closing** (line 26):
```markdown
In general, if you can modify an aspect of how something looks through the Viewer, you are actually modifying the blueprint.
```

**Improvements:**
- Removed technical caveat/parenthetical
- Simplified to single clear statement
- Capitalized "Viewer" for consistency

---

#### 4. "current, default, and heuristics blueprints" → "Reset behavior: heuristic vs default"

**Old Section Header** (line 49):
```markdown
## Current, default, and heuristics blueprints
```

**New Section Header** (line 41):
```markdown
## Reset Behavior: Heuristic vs Default
```

**Improvements:**
- Focuses on user action (reset behavior) rather than three abstract types
- More task-oriented header
- Simpler conceptual model (2 types of reset vs 3 types of blueprints)

**Old Image** (lines 51-52):
```markdown
<!-- source: Rerun Design System/Documentation schematics -->
<img src="https://static.rerun.io/fe1fcf086752f5d7cdd64b195fb3a6cb99c50737_current_default_heuristic.png" width="550px">
```

**New Image** (lines 53-56):
```markdown
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/fe1fcf086752f5d7cdd64b195fb3a6cb99c50737_current_default_heuristic.png">
  <img src="https://static.rerun.io/fe1fcf086752f5d7cdd64b195fb3a6cb99c50737_current_default_heuristic.png" width="550px" alt="Current, default, and heuristic blueprints">
</picture>
```

**Improvements:**
- Image moved to after the explanation (better flow)
- Added responsive `<picture>` element for mobile support
- Added alt text for accessibility
- Removed HTML comment

**Old Content** (lines 54-58):
```markdown
Blueprints may originate from multiple sources.

- The "current blueprint" for a given application ID is the one that is used by the Viewer to display data at any given time. It is updated for each change made to the visualization within the viewer, and may be saved to a blueprint file at any time.
- The "default blueprint" is a snapshot that is set or updated when a blueprint is received from code or loaded from a file. The current blueprint may be reset to default blueprint at any time by using the "reset" button in the blueprint panel's header.
- The "heuristic blueprint" is an automatically-produced blueprint based on the recording data. When no default blueprint is available, the heuristic blueprint is used when resetting the current blueprint. It is also possible to reset to the heuristic blueprint in the selection panel after selecting an application.
```

**New Content** (lines 43-51):
```markdown
The Viewer provides two types of blueprint reset, accessible from the blueprint panel:

### Reset to Heuristic Blueprint
This generates a new blueprint automatically based on your current data. The Viewer analyzes what you've logged and creates an appropriate layout using built-in heuristics. This is useful when you want to start fresh and let Rerun figure out a reasonable layout.

### Reset to Default Blueprint
This returns to your programmatically specified blueprint (sent from code) or a saved blueprint file (`.rbl`). If you've sent a blueprint using `rr.send_blueprint()` or loaded a `.rbl` file, this becomes your "default." The reset button in the blueprint panel will restore this default whenever you need it.

When no default blueprint has been set, the reset button will use the heuristic blueprint instead.
```

**Improvements:**
- Removed "current blueprint" as a separate concept (simplified mental model)
- Created subsections for each reset type (better structure)
- Added context about when to use each type ("This is useful when…")
- Added code example (`rr.send_blueprint()`)
- Added file type mention (`.rbl`)
- Explained fallback behavior explicitly
- More conversational, less technical tone
- Added actionable information about WHERE to find reset (blueprint panel)

---

#### 5. New SECTION: "Three ways to work with blueprints"

**Location:** Lines 58-103

**Entire Section** (new content):
```markdown
## Three Ways to Work with Blueprints

There are three complementary approaches to creating and modifying blueprints:

### 1. Interactively
Modify blueprints directly in the Viewer UI:
-   Drag and drop views to rearrange them
-   Add new views or containers with the "+" button
-   Split views horizontally, vertically, or into grids
-   Change container types (Grid, Horizontal, Vertical, Tabs)
-   Rename views and containers
-   Show, hide, or remove elements

This is the fastest way to experiment with layouts. See [Configure the Viewer](../getting-started/configure-the-viewer.md) for a complete guide.

### 2. Save and Load Files
Save your blueprint configuration to `.rbl` files:
-   Use "Save blueprint…" from the file menu to save your current layout
-   Load blueprints with "Open…" or by dragging `.rbl` files into the Viewer
-   Share blueprint files with teammates to ensure everyone sees data the same way
-   Reuse blueprints across sessions and different recordings (with the same Application ID)

Blueprint files are portable and can be version-controlled alongside your code.

### 3. Programmatically
Write blueprint code that configures the Viewer automatically:
-   Define layouts in Python using `rerun.blueprint` APIs
-   Send blueprints with `rr.send_blueprint()` or via `default_blueprint` parameter
-   Generate layouts dynamically based on your data
-   Perfect for creating consistent views for specific debugging scenarios

For example, you might send different blueprints automatically based on detected issues:
```python
import rerun.blueprint as rrb

if robot_error:
    # Show diagnostic views for debugging
    blueprint = rrb.Grid(
        rrb.Spatial3DView(name="Robot view", origin="/world/robot"),
        rrb.TextLogView(name="Error Logs", origin="/diagnostics"),
        rrb.TimeSeriesView(name="Sensor Data", origin="/sensors"),
    )
    rr.send_blueprint(blueprint, make_active=True)
```

See [Configure the Viewer](../getting-started/configure-the-viewer.md#programmatic-blueprints) for detailed examples.
```

**Purpose:**
This entirely new section provides a high-level overview of the three approaches to working with blueprints, serving as a roadmap for users.

**Improvements:**
- Organizes approaches into clear, numbered categories
- Provides concrete examples for each approach
- Includes actual code example for programmatic approach
- Links to detailed documentation for each method
- Helps users choose the right approach for their needs
- Makes the documentation more actionable and practical

---

#### 6. New SECTION: "Common use cases"

**Location:** Lines 105-125

**Entire Section** (new content):
```markdown
## Common Use Cases

### Debugging Specific Scenarios
Create blueprints optimized for diagnosing particular issues. For example, when debugging robot perception, you might want a blueprint that shows:
-   The camera view in 2D
-   The 3D world with detected objects
-   Detection confidence scores in a time series chart
-   Error logs in a text panel

### Sharing Layouts with Teams
Save a blueprint file and share it with your team. Everyone loading that blueprint with matching recordings will see the data the same way, making it easier to discuss findings and collaborate.

### Templating for Different Data Types
Create different blueprint templates for different types of recordings. For example:
-   A blueprint for autonomous vehicle data that focuses on map views and sensor fusion
-   A blueprint for robotics manipulation that emphasizes joint angles and gripper cameras
-   A blueprint for computer vision that shows side-by-side comparisons of different models

### Dynamic Viewer Configuration
Generate blueprints programmatically based on runtime conditions. For instance, automatically create one view per detected anomaly, or adjust the layout based on how many data sources are active.
```

**Purpose:**
Entirely new section providing practical, real-world examples of how to use blueprints effectively.

**Improvements:**
- Helps users understand practical applications
- Provides concrete scenarios (robot debugging, autonomous vehicles, etc.)
- Makes documentation more relatable and actionable
- Shows the "why" not just the "how"
- Inspires users with possibilities

---

#### 7. "what is a blueprint" + "Blueprint architecture motivation" → "Blueprint architecture"

**Old Sections:**
- "What is a blueprint" (lines 60-75)
- "Blueprint architecture motivation" (lines 93-108)

**New Section Header** (line 127):
```markdown
## Blueprint Architecture
```

**Old "What is a blueprint" Content** (lines 62-75):
```markdown
Under the hood, the blueprint is just data. It is represented by a
[time-series ECS](./entity-component.md), just like a recording. The only
difference is that it uses a specific set of blueprint archetypes and a special
blueprint timeline. Note that even though the blueprint may be sent over the
same connection, blueprint data is kept in an isolated store and is not mixed
with your recording data.

Although the Rerun APIs for working with blueprint may look different from the
regular logging APIs, they are really just syntactic sugar for logging a
collection of blueprint-specific archetypes to a separate blueprint stream.

Furthermore, when you make any change to the Viewer in the UI, what is actually
happening is the Viewer is creating a new blueprint event and adding it to the
end of the blueprint timeline in the blueprint store.
```

**New Introduction** (lines 129-130):
```markdown
Under the hood, blueprints are just data—structured using the same [Entity Component System](./entity-component.md) as your recordings, but with blueprint-specific archetypes and a separate blueprint timeline. This architecture provides several advantages:
```

**Improvements:**
- Combined two separate sections into one
- Condensed technical explanation
- Added em dash for better flow
- Made link text more descriptive ("Entity Component System" vs "time-series ECS")
- Transitions immediately to advantages (better structure)

**Old "Blueprint architecture motivation"** (lines 95-108):
```markdown
Although this architecture adds some complexity and indirection, the fact that
the Viewer stores all of its meaningful frame-to-frame state in a structured
blueprint data-store has several advantages:

-   Anything you modify in the Viewer can be saved and shared as a blueprint.
-   A blueprint can be produced programmatically using just the Rerun SDK without
    a dependency on the Viewer libraries.
-   The blueprint is capable of representing any data that a recording can
    represent. This means that blueprint-sourced data
    [overrides](visualizers-and-overrides.md#Per-entity-component-override) are
    just as expressive as any logged data.
-   The blueprint is actually stored as a full time-series, simplifying future
    implementations of things like snapshots and undo/redo mechanisms.
-   Debugging tools for inspecting generic Rerun data can be used to inspect
    internal blueprint state.
```

**New Advantages List** (lines 130-134):
```markdown
-   **Anything you modify in the Viewer can be saved and shared** as a blueprint file
-   **Blueprints can be produced programmatically** using just the Rerun SDK without depending on the Viewer
-   **Blueprint data is fully expressive**, enabling [blueprint overrides](visualizers-and-overrides.md#per-entity-component-override) that are as powerful as logged data
-   **The full time-series nature** simplifies future features like snapshots and undo/redo
-   **Debugging tools for Rerun data** can inspect blueprint state just like recording data
```

**Improvements:**
- Removed apologetic introduction ("Although this adds complexity…")
- Made each benefit bold for emphasis and scannability
- Simplified and shortened each point
- Added "as a blueprint file" for clarity
- Changed "dependency on the Viewer libraries" to "depending on the Viewer" (simpler)
- Changed "simplifying future implementations" to "simplifies future features" (more concrete)
- Removed unnecessary details while preserving key information

---

#### 8. "viewer operation" → "Viewer operation" (subsection)

**Old Section** (lines 77-91):
```markdown
## Viewer operation

Outside of caching that exists primarily for performance reasons, the viewer
persists very little state frame-to-frame. The goal is for the output of the
Viewer to be a deterministic function of the blueprint and the recording.

Every frame, the Viewer starts with a minimal context of an "active" blueprint,
and an "active" recording. The Viewer then uses the current revision on the
blueprint timeline to query the container and view archetypes from the
blueprint store. The view archetypes, in turn, specify the paths types
that need to be queried from the recording store in order to render the views.

Any user interactions that modify the blueprint are queued and written back to
the blueprint using the next revision on the blueprint timeline.
```

**New Subsection** (lines 136-145):
```markdown
### Viewer Operation

The Viewer is designed to be deterministic. Every frame, the Viewer:
1. Takes the active blueprint and active recording
2. Queries container and view archetypes from the blueprint at the current blueprint timeline revision
3. Uses those view specifications to query the data needed from the recording
4. Renders the results
5. Queues any user interactions as new blueprint events on the blueprint timeline

This means the Viewer output is a deterministic function of the blueprint and the recording, with minimal persisted state between frames.
```

**Improvements:**
- Changed from ## to ### (now a subsection under "Blueprint Architecture")
- Removed verbose introduction about caching
- Started with the key principle ("designed to be deterministic")
- Converted prose description into numbered steps
- Much easier to follow the process flow
- Moved summary sentence to the end
- More concise overall (14 lines → 10 lines)

---

#### 9. New SECTION: "Next steps"

**Location:** Lines 147-152

**Entire Section** (new content):
```markdown
## Next Steps

-   **Learn to use blueprints**: See [Configure the Viewer](../getting-started/configure-the-viewer.md) for hands-on tutorials covering interactive, file-based, and programmatic workflows
-   **Understand the UI**: Check the [Blueprint Panel Reference](../reference/viewer/blueprint.md) for details on UI controls
-   **Customize visualizations**: Learn about [Visualizers and Overrides](visualizers-and-overrides.md) for advanced per-entity customization
-   **Explore the API**: Browse the [Blueprint API Reference](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) for programmatic control (Python)
```

**Purpose:**
Entirely new section providing clear navigation to related documentation.

**Improvements:**
- Helps users continue their learning journey
- Provides logical next steps after understanding concepts
- Links to practical tutorials and reference documentation
- Bold labels make it easy to scan
- Improves overall documentation navigation

---

### Minor formatting changes in blueprint.md

1. **Capitalization**: "viewer" → "Viewer" (consistent capitalization throughout)
2. **Line breaks**: Multiple long paragraphs broken into shorter sentences
3. **Bold emphasis**: Key terms and phrases made bold for emphasis
4. **Link text**: Made more descriptive (e.g., "Entity Component System" instead of "time-series ECS")

---

## content/getting-started/configure-the-viewer.md changes

This file underwent a massive expansion from 23 lines to 598 lines (+2500% growth). The old file was a brief overview with links to sub-pages. The new file is a comprehensive, self-contained tutorial.

### Structural changes

**Old Structure:**
- Brief introduction (4 lines)
- List of what blueprint controls (5 lines)
- Links to three sub-pages (3 lines)

**New Structure:**
- Introduction (8 lines)
- Interactive Configuration (160+ lines with screenshots)
- Save and Load Blueprint Files (30+ lines)
- Programmatic Blueprints (400+ lines with complete code tutorial)

### Title change

**Old:** `title: Configure the viewer`
**New:** `title: Configure the Viewer`

**Improvement:** Consistent capitalization

### Introduction changes

**Old Introduction** (lines 5-11):
```markdown
By default, the Rerun Viewer uses heuristics to automatically determine an appropriate
layout given the data that you provide. However, there will always be situations
where the heuristic results don't match the needs of a particular use-case.

Fortunately, almost all aspects of the Viewer can be configured via the [Blueprint](../reference/viewer/blueprint.md).

The Viewer Blueprint completely determines:
```

**New Introduction** (lines 5-11):
```markdown
By default, the Rerun Viewer uses heuristics to automatically determine an appropriate layout for your data. However, you'll often want precise control over how your data is displayed. Blueprints give you complete control over the Viewer's layout and configuration.

For a conceptual understanding of blueprints, see [Blueprints](../concepts/blueprint.md).

This guide covers three complementary ways to work with blueprints:
```

**Improvements:**
- More concise and direct language
- Changed "there will always be situations where" to "you'll often want" (more positive framing)
- Removed "Fortunately" (unnecessary qualifier)
- Added link to conceptual documentation
- Set expectations for what the guide covers
- More conversational tone

**Old List** (lines 13-19):
```markdown
-   What contents are included in each view
-   The type of view used to visualize the data
-   The organizational layout and names of the different view panels and containers
-   Configuration and styling properties of the individual data visualizers

There are a few different ways to work with Blueprints:
```

**New List** (lines 9-11):
```markdown
- **[Interactive configuration](#interactive-configuration)**: Modify layouts directly in the Viewer UI
- **[Save and load blueprint files](#save-and-load-blueprint-files)**: Share layouts using `.rbl` files
- **[Programmatic blueprints](#programmatic-blueprints)**: Control layouts from code
```

**Improvements:**
- Removed technical list (moved to concepts page)
- Replaced with actionable table of contents
- Added anchor links for easy navigation
- More user-task-oriented
- Clear descriptions of each approach

### Interactive configuration section (NEW CONTENT)

**Lines 13-190**: This entire section is new content that consolidates and expands on the deleted `interactively.md` file.

**Key Additions:**

1. **Viewer Overview** (lines 15-26)
   - Added descriptive list of viewer components
   - Kept the overview image
   - Made image responsive with `<picture>` element
   - Added explanation of each panel

2. **Configuring the View Hierarchy** (lines 28-72)
   - Added introduction explaining container types
   - Listed all four container types with descriptions
   - Organized sub-sections with #### headers

3. **Add New Containers or Views** (lines 30-41)
   - Clear instructions with screenshots
   - Multiple methods described

4. **Rearrange Views and Containers** (lines 43-49)
   - Drag and drop instructions
   - Screenshots showing the action

5. **Show, Hide, or Remove Elements** (lines 51-61)
   - Separate sub-sections for show/hide vs remove
   - Clear visual examples

6. **Rename Views and Containers** (lines 63-68)
   - Step-by-step instructions
   - Screenshot showing the UI

7. **Change Container Type** (lines 70-75)
   - Clear instructions with dropdown location
   - Visual example

8. **Using Context Menus** (lines 77-84)
   - Explanation of right-click functionality
   - Multi-selection capability highlighted

9. **Configuring View Content** (lines 86-127)
   - New major subsection
   - Multiple ways to modify view content
   - Query editor explanation
   - Creating views from entities

10. **Overriding Visualizers and Components** (lines 129-152)
    - Advanced customization options
    - Screenshots showing the UI
    - Link to detailed documentation

**Improvements over old interactively.md:**
- Better organization with clear hierarchy
- More context and explanation
- Consistent formatting
- Better integration into overall guide
- Progressive disclosure (basic → advanced)

### Save and load blueprint files section (NEW CONTENT)

**Lines 194-226**: This section consolidates and expands content from the deleted `save-and-load.md` file.

**New Content Structure:**

1. **Introduction** (lines 196-198)
   - Clear value proposition
   - Sets context for the section

2. **Saving a Blueprint** (lines 200-206)
   - Step-by-step instructions
   - Screenshot showing menu location
   - Note about file portability

3. **Loading a Blueprint** (lines 208-214)
   - Multiple loading methods
   - Important note about Application ID matching
   - Link to conceptual explanation

4. **Sharing Blueprints** (lines 216-226)
   - Numbered workflow for sharing
   - Bullet list of use cases (debugging, presentations, data analysis)

**Improvements over old save-and-load.md:**
- More comprehensive (25 lines → ~30 lines)
- Added sharing workflow
- Added use cases for context
- Better organized
- More actionable

### Programmatic blueprints section (NEW CONTENT)

**Lines 228-598**: This massive section (~370 lines) is a complete, hands-on tutorial with a working example.

**Content Breakdown:**

1. **Introduction** (lines 230-236)
   - Explains when to use programmatic blueprints
   - Lists benefits (dynamic, consistent, automated)
   - Sets expectations

2. **Getting Started Example** (lines 238-240)
   - Introduces the stocks example
   - Progressive tutorial approach

3. **Setup** (lines 242-254)
   - Installation instructions for both platforms
   - Virtual environment setup
   - Dependencies listed

4. **Basic Script** (lines 256-322)
   - Complete, runnable code
   - Imports
   - Helper functions for styling
   - Main function skeleton

5. **Step-by-Step Blueprint Building** (lines 324-598)
   - Multiple progressive examples
   - Starting simple (basic layout)
   - Adding complexity (splits, tabs, custom queries)
   - Advanced features (time ranges, visual bounds)
   - Each example builds on the previous

**Code Examples Included:**

The tutorial includes complete Python code for:
- Setting up the environment
- Logging stock data
- Creating basic blueprint layouts
- Using horizontal/vertical splits
- Using tab containers
- Configuring time series views
- Setting time ranges
- Setting visual bounds
- Multiple complete examples users can run

**Improvements:**
- Completely self-contained tutorial
- Runnable code throughout
- Progressive complexity
- Real-world example (stock market data)
- Clear explanations of each concept
- Multiple approaches shown

---

## Cross-Reference updates

### File: content/concepts/apps-and-recordings.md

**Line 20:**

**Old:**
```markdown
Note that [blueprints](../howto/configure-viewer-through-code.md) are recordings too, and by convention are stored in binary `.rbl` files.
```

**New:**
```markdown
Note that [blueprints](blueprint.md) are recordings too, and by convention are stored in binary `.rbl` files.
```

**Improvement:**
- Updated link to point to concepts page instead of howto
- More appropriate link target (concepts vs how-to)

---

### Files with link updates

The following files had links updated to point to the new consolidated documentation structure:

1. **content/getting-started/navigating-the-viewer.md**
   - Updated blueprint references to new locations

2. **content/howto/configure-viewer-through-code.md**
   - Updated internal links

3. **content/howto/integrations/embed-notebooks.md**
   - Updated blueprint references

4. **content/howto/visualization/geospatial-data.md**
   - Updated blueprint references

5. **content/reference/dataframes.md**
   - Updated blueprint references

6. **content/reference/viewer/blueprint.md**
   - Updated cross-references

7. **content/reference/viewer/viewport.md**
   - Updated cross-references

**Nature of Changes:**
- Link target updates (old sub-pages → new main pages)
- Ensuring all links point to correct consolidated locations
- Maintaining documentation integrity after restructuring

---

## Summary statistics

### Content growth
- **concepts/blueprint.md**: 109 lines → 153 lines (+40%)
- **getting-started/configure-the-viewer.md**: 23 lines → 598 lines (+2500%)
- **Total new content**: ~575 lines of comprehensive tutorial material

### Files changed
- **2 major rewrites**: blueprint.md, configure-the-viewer.md
- **5 files deleted**: Sub-pages consolidated
- **8 files updated**: Cross-reference fixes

### Documentation improvements categories

1. **Organization**
   - Consolidated 5 separate files into 2 comprehensive guides
   - Better information architecture
   - Clearer navigation with table of contents
   - Logical progression from concepts to practice

2. **Clarity**
   - Shorter sentences
   - More direct language
   - Less technical jargon
   - Better use of examples

3. **Scannability**
   - More bullet lists (vs prose)
   - Bold emphasis on key terms
   - Better headers and sub-headers
   - Numbered steps for procedures

4. **Completeness**
   - Added 370-line hands-on tutorial
   - Added practical use cases
   - Added code examples
   - Added multiple approaches to same task

5. **Accessibility**
   - Responsive images with `<picture>` elements
   - Alt text on images
   - Better link text descriptions
   - Multiple entry points for information

6. **Usability**
   - More actionable content
   - Clear next steps
   - Progressive disclosure (basic → advanced)
   - Task-oriented organization

7. **Tone**
   - More conversational
   - More encouraging
   - Less defensive/apologetic
   - More confident and direct

---

## Conclusion

This documentation update represents a comprehensive overhaul focused on:
- **Consolidation**: Reducing fragmentation by merging related content
- **Expansion**: Adding extensive practical examples and tutorials
- **Clarification**: Making implicit behaviors explicit and simplifying explanations
- **User-centricity**: Organizing content around user tasks and needs rather than technical architecture

All changes are pure documentation improvements with no indication of underlying functionality changes to the blueprint system itself.
