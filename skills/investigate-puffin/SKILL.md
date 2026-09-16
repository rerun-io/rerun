---
name: investigate-puffin
description: Investigate Puffin profiler recordings (.puffin files), find slow scopes, and compare before/after traces using the `rerun dump-puffin` command and bundled analysis helpers.
---

# Investigate puffin traces

Analyze a `.puffin` recording from an application instrumented with Puffin.
Use the recording path supplied by the user.

## Setup

Requires the `rerun` CLI (Rerun 0.38 or later), Python 3.10 or later, and jq.
The Python helpers use only the standard library.
Set `PUFFIN_SKILL_DIR` to the absolute directory containing this `SKILL.md`.
In Claude Code, you can use `export PUFFIN_SKILL_DIR="${CLAUDE_SKILL_DIR}"`; in other agents, resolve the directory from the installed skill location.

Dump the trace to a file and check scope registration:

```bash
rerun dump-puffin <path> > /tmp/puffin.json
jq '.scopes | length' /tmp/puffin.json
```

If no (recent enough) `rerun` binary is on the path but you are in a Rerun checkout, run the sub-command from source instead:

```bash
cargo run --quiet -p rerun-cli --no-default-features -- dump-puffin <path> > /tmp/puffin.json
```

Always dump to a file.
`.puffin` traces can produce many MB of JSON, so query the saved JSON instead of reading it all into context.

If `.scopes | length` is **0**, names fall back to `scope#<id>` and source locations are missing.
Ask for a new capture that includes scope registration if names are needed for the investigation.
If another capture is not feasible, proceed with dynamic `data` labels and explain the limits.
Check the registration table after exporting: do not assume that every capture tool or version preserves it.

### How traces are captured

Use the application's Puffin capture or export support to save a `.puffin` file.
For applications serving profiling data through `puffin_http`, connect a compatible `puffin_viewer` and export the recording.
The dumper uses Puffin 0.20; captures from incompatible format versions may require a matching parser.

Rerun-specific examples:

- **Native viewer / CLI:** enable the built-in Puffin server from the UI.
- **Python SDK:** set `RERUN_PUFFIN=1` before importing `rerun` to stream profiling data to `puffin_viewer`.
- **Rust binaries using `re_tracing`:** keep a `re_tracing::Profiler` alive for the capture and call its `start()` method with server support enabled.
  It starts a server and attempts to launch `puffin_viewer`.

## Investigate with jq

The JSON layout:

```
{
  "file": "preview.puffin",
  "scopes": { "<id>": { name, function, file, line, kind } },   # may be empty — see Setup
  "frames": [
    {
      "frame_index": …,
      "range_ns": [start, end],
      "duration_ns": …,
      "num_scopes": …,
      "threads": [
        {
          "name": "main",
          "num_scopes": …, "depth": …, "range_ns": […],
          "scopes": [ { id, name, start_ns, duration_ns, data, children: […] } ]
        }
      ]
    }
  ]
}
```

`name` on each scope is resolved from the `scopes` map.
`data` is the dynamic per-call string, such as an event type or a mesh name.

### Recipes

**Footgun:** any `.. | objects | select(.name? == …)` pattern MUST also `select(has("duration_ns"))`.
`..` walks every object in the tree, including entries in the top-level `scopes` registration map, which have `.name` but no `.duration_ns`.
Without the guard you will hit `null / 1e6` errors or sort against `null`.
Every recipe below includes the guard, keep it when adapting them.

**Prefer `find_scopes.py` for name/data lookups.** If you just want "slowest matches of X", "total time grouped by name" or "all scopes matching X on thread Y", reach for `find_scopes.py` instead of writing jq, it's shorter and bakes the guard in.
See "Looking for a scope by prefix" below.
Use the jq recipes here when you need something the script does not cover (children of a specific scope, cross-thread windows, time-ordered traversal).

```bash
# overall shape — sorted slowest first so the bad frames jump out
jq '.frames | map({i: .frame_index, ms: (.duration_ns / 1e6), scopes: .num_scopes}) | sort_by(-.ms)' /tmp/puffin.json

# per-thread totals for the slowest frame
jq '.frames | sort_by(-.duration_ns) | .[0].threads | map({name, num_scopes, depth, ms: ((.range_ns[1] - .range_ns[0]) / 1e6)})' /tmp/puffin.json

# top 10 slowest individual scopes (recursive descent over the whole tree)
# also doable as: find_scopes.py /tmp/puffin.json '' --top 10
jq '[.. | objects | select(has("duration_ns") and has("name"))]
    | sort_by(-.duration_ns) | .[0:10]
    | map({name, data, ms: (.duration_ns / 1e6)})' /tmp/puffin.json

# total time per scope name (best-effort: groups identically-named scopes)
# usually easier as: find_scopes.py /tmp/puffin.json '' --aggregate
jq '[.. | objects | select(has("duration_ns") and has("name"))]
    | group_by(.name)
    | map({name: .[0].name, total_ms: ((map(.duration_ns) | add) / 1e6), count: length})
    | sort_by(-.total_ms) | .[0:15]' /tmp/puffin.json

# total time per (name, data) pair, useful when names are scope#NN
# usually easier as: find_scopes.py /tmp/puffin.json '' --aggregate
jq '[.. | objects | select(has("duration_ns") and has("name"))]
    | group_by([.name, .data])
    | map({name: .[0].name, data: .[0].data, total_ms: ((map(.duration_ns) | add) / 1e6), count: length})
    | sort_by(-.total_ms) | .[0:15]' /tmp/puffin.json

# children of the slowest instance of a named scope
jq --arg n WindowEvent::RedrawRequested '
    [.. | objects | select(has("duration_ns") and (.name? == $n or .data? == $n))]
    | sort_by(-.duration_ns) | .[0]
    | .children | map({name, data, ms: (.duration_ns / 1e6)}) | sort_by(-.ms)' /tmp/puffin.json

# children of a named scope, ORDERED IN TIME — gaps between (start + duration) and the next start
# are unprofiled wall-clock, possibly parallel work dispatched without a surrounding wait scope.
# See "Watch for parallel gaps" below.
jq --arg n WindowEvent::RedrawRequested '
    [.. | objects | select(has("duration_ns") and (.name? == $n or .data? == $n))]
    | sort_by(-.duration_ns) | .[0]
    | .children | sort_by(.start_ns)
    | map({name, data, start_ns, ms: (.duration_ns / 1e6)})' /tmp/puffin.json

# per-call durations for one specific scope name — use this to sanity-check whether
# a "big delta" in a comparison is a real shift or a single outlier frame.
jq --arg n Tessellator::tessellate_shapes '
    [.. | objects | select(has("duration_ns") and (.name? == $n or .data? == $n))]
    | map({ms: (.duration_ns / 1e6), data})' /tmp/puffin.json

# what was running on any thread during a [start, end] window in nanoseconds
# (use this to find where a "missing" gap on main went — usually rayon-N)
jq --argjson lo 1777363867918216917 --argjson hi 1777363867919755709 '
    [.frames[].threads[]
        | {thread: .name, scopes: [.scopes[] | .. | objects
            | select(has("duration_ns") and .start_ns >= $lo and (.start_ns + .duration_ns) <= $hi)]}
        | select(.scopes | length > 0)]
    | map({thread, count: (.scopes | length),
           top: (.scopes | sort_by(-.duration_ns) | .[0:5]
                 | map({name, data, ms: (.duration_ns/1e6)}))})' /tmp/puffin.json

# resolve a scope id to its source location (when scopes map is populated)
jq '.scopes["74"]' /tmp/puffin.json
```

### Watch for parallel gaps

`puffin` profiles whatever you wrap in `profile_function!` / `profile_scope!`.
**`rayon::par_iter` calls do NOT auto-add a wait scope** — only `re_tracing::profile_wait!` does.
So when code dispatches parallel work without a `profile_wait!`, the main thread shows a *gap*: a stretch of wall-clock between two scopes with no scope covering it.
You will usually only spot this by reading the time-ordered-children output above and noticing `next.start_ns - (prev.start_ns + prev.duration_ns)` is multiple ms.

When you see such a gap on main, note the `[gap_start, gap_end]` window in nanoseconds and run the cross-thread window recipe above with those bounds.
Check worker threads, such as `rayon-N`, for activity in that window.
A gap can also represent uninstrumented work, I/O, or scheduler delay.

### Comparing two traces (before/after)

```bash
# dump the second trace
rerun dump-puffin <new-path> > /tmp/puffin2.json

# joined per-(name, data) comparison sorted by |Δtotal|
# --sort delta is also the default: largest absolute change first.
# Positive deltas mean B took longer; negative deltas mean B took less time.
# Increase --top to inspect smaller changes of either sign.
python3 "${PUFFIN_SKILL_DIR}/compare_traces.py" /tmp/puffin.json /tmp/puffin2.json --top 50

# narrow to a specific area (filters by name/data prefix)
python3 "${PUFFIN_SKILL_DIR}/compare_traces.py" /tmp/puffin.json /tmp/puffin2.json --filter system_execution

# hide rows below 0.5 ms total in both (kill noise)
python3 "${PUFFIN_SKILL_DIR}/compare_traces.py" /tmp/puffin.json /tmp/puffin2.json --hide-below 0.5
```

Focus on **per-call cost when count is unchanged**, not just "did it run more times." That's what reveals lock contention or nested-pool pessimizations.

Before reporting any per-call delta as a real change, dump the per-call durations of that scope in both traces using the per-call recipe above.
Traces are usually short (a handful of frames) and a single spike frame is enough to swing a per-call average by 2-3x.

**Caveat:** if one trace has scope registration and the other doesn't, most rows show as `new` or `gone` because names won't match.
Check `.scopes | length` on both before comparing.

### Looking for a scope by prefix

For "find every scope whose name or data starts with X", use the helper script:

```bash
# slowest individual matches
python3 "${PUFFIN_SKILL_DIR}/find_scopes.py" /tmp/puffin.json Redraw --top 20

# group by (name, data), see total time per group
python3 "${PUFFIN_SKILL_DIR}/find_scopes.py" /tmp/puffin.json query --aggregate

# narrow by frame or thread
python3 "${PUFFIN_SKILL_DIR}/find_scopes.py" /tmp/puffin.json scan --thread main --frame 2578

# match only the dynamic data field
python3 "${PUFFIN_SKILL_DIR}/find_scopes.py" /tmp/puffin.json on_input --field data
```

## Analyzing the data

A trace usually only covers a handful of frames.
Look out for outlier frames and outlier per-call instances.
If a delta is dominated by one spike, do not quote the averaged speedup as a clean number, call out the spike on its own.

When comparing two traces, ask the user for the git diff between them and use it to judge which timing deltas are plausibly attributable to the change.
If something differs in a way the diff cannot explain, check with the user whether both captures are from the same scenario.

## Cross-check with the source code

Once a slow scope is identified, go look at the actual code:

- If `.scopes[<id>]` is populated, it gives `file` and `line` directly.
`Read` that file at that line and walk into the called functions.
- If only `data` / `name` is available, grep the repo for the literal string.
The macros are `puffin::profile_function!()`, `puffin::profile_scope!("name")`, `puffin::profile_scope!("name", data_expr)`, plus the `re_tracing::profile_*!` wrappers.
`data_expr` is whatever was passed at the call site, so the literal is usually findable.
- If the user points at a specific function ("is `apply_many` actually the bottleneck?"), filter the JSON to that scope's subtree, report its self-time vs children-time, then read the implementation.

## Notes

- Reuse `/tmp/puffin.json` (and `/tmp/puffin2.json` for comparisons).
Don't re-run the dumper unless the user gives you a different file.
- A scope's `start_ns` is absolute monotonic-clock nanoseconds, not relative to the frame.
Subtract `start_ns + duration_ns` of one scope from `start_ns` of the next sibling to get gap size on the same thread.
