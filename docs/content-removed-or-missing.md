# Content removed or missing from blueprint documentation

This document identifies content that existed in the old documentation but is either **removed** or **missing** from the new consolidated documentation.

## Executive summary

During the consolidation of blueprint documentation, several important pieces of content from the deleted files were **not fully transferred** to the new documentation. Specifically:

1. **API reference material** from `configure-viewer-through-code.md` was removed
2. **Cross-language blueprint reuse** information from `reuse-blueprints.md` was removed
3. **Advanced API parameters** documentation was removed
4. **Historical context** about API limitations was removed

## Detailed analysis by source file

---

## 1. From `content/howto/visualization/configure-viewer-through-code.md`

This file (288 lines) contained substantial API reference content that is **NOT in the new documentation**.

### Missing section: "Blueprint API overview"

**Old Location:** Lines 12-46
**New Location:** Does not exist

**Old Content:**
```markdown
## Blueprint API overview

All blueprint APIs are in the [`rerun.blueprint`](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) namespace. In our Python examples, we typically import this using the `rrb` alias:

```python
import rerun.blueprint as rrb
```

The Python blueprint API is declarative and object-centric. There are 3 main types of blueprint objects you will
encounter:

-   `Blueprint`: The root object that represents the entire Viewer layout.
-   `Container`: A layout object that contains other containers or views.
-   `View`: A view object that represents a single view of the data.

Both containers and views should be used via typed subclasses instead.:

-   `Container` has subclasses: `Horizontal`, `Vertical`, `Grid`, and `Tabs`.
-   `View` has subclasses: `BarChartView`, `Spatial2DView`, `Spatial3DView`, `TensorView`,
    `TextDocumentView`, `TextLogView`, and `TimeSeriesView`.

These paths can be combined hierarchically to create a complex Viewer layout.

For example:

```python
my_blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.BarChartView(),
        rrb.Vertical(
            rrb.Spatial2DView(),
            rrb.Spatial3DView(),
        ),
    ),
)
```
```

**Impact:** Users no longer have a concise API overview explaining the three main types and their subclasses.

---

### Missing section: "Sending the blueprint to the viewer"

**Old Location:** Lines 49-72
**New Location:** Partially mentioned, not fully explained

**Old Content:**
```markdown
## Sending the blueprint to the Viewer

To provide a blueprint, simply pass it to either `init` or `connect_grpc` using the `default_blueprint`
parameter.

Using `init` with `spawn=True`:

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint", spawn=True, default_blueprint=my_blueprint)
```

Or if you use `connect_grpc` separate from `init`:

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint")

...

rr.connect_grpc(default_blueprint=my_blueprint)
```
```

**New Content:** The new tutorial only shows `rr.send_blueprint()` - the `default_blueprint` parameter approach is not documented.

**Impact:** Users don't know they can pass blueprints via the `default_blueprint` parameter to `init()` or `connect_grpc()`.

---

### Missing section: "Activating the default blueprint"

**Old Location:** Lines 74-91
**New Location:** Does not exist

**Old Content:**
```markdown
## Activating the default blueprint

Just like the Viewer can store many different recordings internally, it can also
store many different blueprints. For each `application_id` in the viewer, are two
particularly important blueprints: the "default blueprint" and the "active blueprint".

When a recording is selected, the active blueprint for the corresponding
`application_id` will completely determine what is displayed by the viewer.

When you send a blueprint to the viewer, it will not necessarily be
activated immediately. The standard behavior is to only update the "default
blueprint" in the viewer. This minimizes the chance that you accidentally
overwrite blueprint edits you may have made locally.

If you want to start using the new blueprint, after sending it, you will need to
click the reset button in the blueprint panel. This resets the active blueprint to the
current default.
```

**Impact:** Users don't understand the critical distinction between "active" and "default" blueprints, or why their programmatic blueprints might not appear immediately.

---

### Missing section: "Always activating the blueprint"

**Old Location:** Lines 93-107
**New Location:** Partially mentioned in passing

**Old Content:**
```markdown
## Always activating the blueprint

If you want to always activate the blueprint as soon as it is received, you can instead use the `send_blueprint`
API. This API has two flags `make_active` and `make_default`, both of which default to `True`.

If `make_active` is set, the blueprint will be activated immediately. Exercise care in using this API, as it can be
surprising for users to have their blueprint changed without warning.

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint", spawn=True)

rr.send_blueprint(my_blueprint, make_active=True)

```
```

**New Content:** The new tutorial uses `rr.send_blueprint()` but doesn't explain the `make_active` and `make_default` parameters or their defaults.

**Impact:** Users don't understand why `rr.send_blueprint()` activates immediately while `default_blueprint=` doesn't.

---

### Missing section: "Customizing views" (detailed explanation)

**Old Location:** Lines 109-187
**New Location:** Partially covered in tutorial examples, but detailed explanations removed

**Old Content Highlights:**

1. **Explanation of default behavior:**
```markdown
Any of the views can be instantiated with no arguments.
By default these views try to include all compatible entities.
```

2. **Three parameters explained:**
```markdown
Beyond instantiating the views, there are 3 parameters you may want to specify: `name`, `origin`, and `contents`.
```

3. **Detailed `origin` explanation:**
```markdown
### `origin`

The `origin` of a view is a generalized "frame of reference" for the view. We think of showing all data
in the view as relative to the `origin`.

By default, only data that is under the `origin` will be included in the view. As such this is one of the most
convenient ways of restricting a view to a particular subtree.

Because the data in the view is relative to the `origin`, the `origin` will be the first entity displayed
in the blueprint tree, with all entities under the origin shown using relative paths.

For Spatial views such as `Spatial2DView` and `Spatial3DView`, the `origin` plays an additional role with respect
to data transforms. All data in the view will be transformed to the `origin` space before being displayed. See [Spaces and Transforms](../../concepts/spaces-and-transforms.md) for more information.
```

4. **Detailed `contents` explanation:**
```markdown
### `contents`

If you need to further modify the contents of a view, you can use the `contents` parameter. This parameter is
a list of [entity query expressions](../../reference/) that are either included or excluded from the
view.

Each entity expressions starts with "+" for inclusion or "-" for an exclusion. The expressions can either be specific entity paths, or may end in a wildcard `/**` to include all entities under a specific subtree.

When combining multiple expressions, the "most specific" rule wins.

Additionally, these expressions can reference `$origin` to refer to the origin of the view.
```

**New Content:** The tutorial shows examples but doesn't provide the conceptual explanations above.

**Impact:** Users see how to use `origin` and `contents` but don't understand the deeper concepts like spatial transforms or the "most specific rule wins" for content expressions.

---

### Missing section: "Implicit conversion"

**Old Location:** Lines 189-226
**New Location:** Does not exist

**Old Content:**
```markdown
## Implicit conversion

For convenience all of the blueprint APIs take a `BlueprintLike` rather than requiring a `Blueprint` object.
Both `View`s and `Containers` are considered `BlueprintLike`. Similarly, the `Blueprint` object can
take a `View` or `Container` as an argument.

All of the following are equivalent:

```python
rr.send_blueprint(rrb.Spatial3DView())
```

```python
rr.send_blueprint(
    rrb.Grid(
        Spatial3DView(),
    )
)
```

```python
rr.send_blueprint(
    rrb.Blueprint(
        Spatial3DView(),
    ),
)

```

```python
rr.send_blueprint(
    rrb.Blueprint(
        rrb.Grid(
            Spatial3DView(),
        )
    ),
)
```
```

**Impact:** Users don't know that views and containers can be passed directly without wrapping them in a `Blueprint()` object.

---

### Missing section: "Customizing the top-level blueprint"

**Old Location:** Lines 228-287
**New Location:** Partially covered, but missing details

**Old Content - Controlling panel state:**
```markdown
### Controlling the panel state

The `Blueprint` controls the default panel-state of the 3 panels: the `BlueprintPanel`, the `SelectionPanel`, and the `TimePanel`. These can be controlled by passing them as additional arguments to the `Blueprint` constructor.

```python
rrb.Blueprint(
    rrb.TimePanel(state="collapsed")
)
```

As an convenience, you can also use the blueprint argument: `collapse_panels=True` as a short-hand for:

```python
rrb.Blueprint(
    rrb.TimePanel(state="collapsed"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.BlueprintPanel(state="collapsed"),
)
```
```

**New Content:** Shows individual panel control but doesn't mention the `collapse_panels=True` shorthand.

---

**Old Content - Controlling auto behaviors:**
```markdown
### Controlling the auto behaviors

The blueprint has two additional parameters that influence the behavior of the viewer:

-   `auto_views` controls whether the Viewer will automatically create views for entities that are not explicitly included in the blueprint.
-   `auto_layout` controls whether the Viewer should automatically layout the containers when introducing new views.

If you pass in your own `View` or `Container` objects, these will both default to `False` so that the Blueprint
you get is exactly what you specify. Otherwise they will default to `True` so that you will still get content (this
matches the default behavior of the Viewer if no blueprint is provided).

This means that:

```python
rrb.Blueprint()
```

and

```python
rrb.Blueprint(
    auto_views=True,
    auto_layout=True
)
```

are both equivalent to the viewer's default behavior.

If you truly want to create an empty blueprint, you must set both values to `False`:

```python
rrb.Blueprint(
    auto_views=False,
    auto_layout=False
),
```
```

**New Content:** The `auto_views` and `auto_layout` parameters are **completely missing** from the new documentation.

**Impact:** Users don't know about these important parameters that control automatic view creation and layout.

---

### Missing context: API limitations note

**Old Location:** Lines 6-10
**New Location:** Does not exist

**Old Content:**
```markdown
As of Rerun 0.15, the state of the [blueprint](../../reference/viewer/blueprint.md) can be directly manipulated using the
Rerun SDK.

In the initial 0.15 release, the APIs are still somewhat limited and only available in the Python SDK.
Future releases will add support for the full scope of blueprint. See issues: [#5519](https://github.com/rerun-io/rerun/issues/5519), [#5520](https://github.com/rerun-io/rerun/issues/5520), [#5521](https://github.com/rerun-io/rerun/issues/5521).
```

**Impact:** This historical context was removed. If the APIs are still Python-only, users should be informed.

---

## 2. From `content/howto/visualization/reuse-blueprints.md`

This file (45 lines) contained information about **cross-language blueprint reuse** that is missing from the new documentation.

### Missing section: "Creating a blueprint file"

**Old Location:** Lines 13-22
**New Location:** Partially covered in new "Save and Load Blueprint Files" section, but missing important details

**Old Content:**
```markdown
## Creating a blueprint file

Blueprint files (`.rbl`, by convention) can currently be created in two ways.

One is to use the Rerun viewer to interactively build the blueprint you want (e.g. by moving panels around, changing view settings, etc), and then using `Menu > Save blueprint` (or the equivalent palette command) to save the blueprint as a file.

The other is to use the [🐍 Python blueprint API](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) to programmatically build the blueprint, and then use the [`Blueprint.save`](https://ref.rerun.io/docs/python/0.19.0/common/blueprint_apis/#rerun.blueprint.Blueprint.save) method to save it as a file:

snippet: tutorials/visualization/save_blueprint
```

**New Content:** The new documentation shows `Blueprint.save()` in one example but doesn't explain it as one of "two ways" to create blueprint files, and doesn't show the snippet reference.

---

### Missing section: "(Re)Using a blueprint file" - cross-language usage

**Old Location:** Lines 24-38
**New Location:** Does not exist

**Old Content:**
```markdown
## (Re)Using a blueprint file

There are two ways to re-use a blueprint file.

The interactive way is to import the blueprint file directly into the Rerun viewer, using either `Menu > Import…` (or the equivalent palette command) or simply by drag-and-dropping the blueprint file into your recording.

The programmatic way works by calling `log_file_from_path`:
* [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
* [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
* [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)

This method allows you to log any file that contains data that Rerun understands (in this case, blueprint data) as part of your current recording:

snippet: tutorials/visualization/load_blueprint
```

**New Content:** The new documentation mentions "Load blueprints with 'Open...' or by dragging .rbl files" but **completely omits** the programmatic `log_file_from_path` approach and cross-language support.

**Impact:** Users working in Rust, C++, or other non-Python languages don't know how to load blueprint files programmatically.

---

### Missing section: "Limitation: dynamic blueprints"

**Old Location:** Lines 40-45
**New Location:** Does not exist

**Old Content:**
```markdown
## Limitation: dynamic blueprints

Sometimes, you might need your blueprint to dynamically react to the data you receive at runtime (e.g. you want to create one view per anomaly detected, and there is no way of knowing how many anomalies you're going to detect until the program actually runs).

The only way to deal with these situations today is to use the [🐍 Python](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) API.
```

**Impact:** Users don't understand the limitation that dynamic blueprints require Python. This might have been resolved, but if not, it should still be documented.

---

## 3. Content from `content/getting-started/configure-the-viewer/interactively.md`

**Status:** ✅ **FULLY MIGRATED**

All content from this file appears to have been successfully migrated to the new "Interactive Configuration" section of `configure-the-viewer.md`. No missing content detected.

---

## 4. Content from `content/getting-started/configure-the-viewer/save-and-load.md`

**Status:** ✅ **MOSTLY MIGRATED** with minor omissions

The core content was migrated, but some minor details were simplified:

**Minor Omission:**
The old version said: "It is not currently possible to change the application ID of a blueprint to use it with a different type of recording."

The new version says: "The blueprint's Application ID must match the Application ID of your recording."

The new version is clearer but loses the explicit statement that you **cannot** change the Application ID.

---

## 5. Content from `content/getting-started/configure-the-viewer/through-code-tutorial.md`

**Status:** ✅ **FULLY MIGRATED** with additions

The tutorial content was successfully migrated. The new version even adds:
- "Saving Blueprints from Code" section (new)
- "Advanced Customization" section with camera settings and time ranges (new)

However, this file originally linked to the howto guide for API reference, which is now removed (see #1 above).

---

## Summary table

| Source File | Lines | Status | Impact |
|------------|-------|--------|---------|
| `interactively.md` | 180 | ✅ Fully migrated | None |
| `save-and-load.md` | 25 | ✅ Mostly migrated | Minor - lost one clarification |
| `through-code-tutorial.md` | 410 | ✅ Fully migrated + enhanced | None |
| `configure-viewer-through-code.md` | 288 | ❌ **Mostly removed** | **HIGH** - Lost API reference material |
| `reuse-blueprints.md` | 45 | ❌ **Mostly removed** | **MEDIUM** - Lost cross-language info |

---

## Recommendations

### Critical missing content (Should be added)

1. **Active vs Default Blueprint distinction** - Critical for understanding blueprint behavior
2. **`make_active` and `make_default` parameters** - Explains why blueprints might not appear
3. **`auto_views` and `auto_layout` parameters** - Important for controlling automatic behavior
4. **`default_blueprint` parameter to `init()`/`connect_grpc()`** - Alternative way to send blueprints
5. **Cross-language blueprint loading with `log_file_from_path()`** - Essential for non-Python users

### Important missing content (Consider adding)

6. **`collapse_panels=True` shorthand** - Convenient API feature
7. **`BlueprintLike` implicit conversion** - Explains flexible API usage
8. **Detailed explanations of `origin` and `contents`** - Deeper conceptual understanding
9. **"Most specific rule wins" for content expressions** - Important behavior detail
10. **Spatial transform role of `origin` in 3D views** - Advanced feature explanation

### Nice-to-Have missing content

11. **Blueprint API overview section** - Quick reference for object types
12. **API limitations note** - If still applicable (Python-only, etc.)
13. **Limitation about dynamic blueprints** - If still applicable

---

## Verification needed

Some removed content may be **outdated**. Before adding back, verify:

1. Are blueprint APIs still Python-only, or available in other languages now?
2. Are there still limitations with dynamic blueprints?
3. Have `auto_views` and `auto_layout` changed or been deprecated?
4. Are the GitHub issues mentioned (#5519, #5520, #5521) still relevant?

---

## Conclusion

While the documentation consolidation improved organization and added valuable tutorial content, it removed significant **API reference material** that users need for advanced blueprint usage. The most critical omissions are:

1. Explanation of active vs default blueprints
2. Parameters controlling blueprint activation and automatic behaviors
3. Cross-language blueprint usage information

These should be restored, potentially in a new "Blueprint API Reference" section or integrated appropriately into the existing tutorial.
