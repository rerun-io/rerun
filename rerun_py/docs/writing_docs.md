# Python docs

A high-level overview of writing and previewing the Rerun Python documentation.

## How docs coverage works

The set of symbols documented at `ref.rerun.io/docs/python/` is derived
automatically from the public-API conventions in the SDK source. There is no
hand-curated index to maintain in lockstep — adding a new public symbol via
the conventions below is sufficient to surface it in the docs.

A symbol is considered public (and gets documented) when any of the following
applies in its module:

- It is listed in the module's `__all__`.
- It is re-exported via `from ._x import Foo as Foo` (PEP 484 redundant alias).
- It is defined in-file with a non-underscore name and the module does not
  define `__all__`.

The conventions are detected by [`griffe`](https://mkdocstrings.github.io/griffe/)
plus the [`griffe-public-redundant-aliases`](https://mkdocstrings.github.io/griffe/extensions/official/public-redundant-aliases/)
extension (configured in `mkdocs.yml`).

### Adding a new public symbol

- **Re-export from a subpackage:** add `from ._impl import Foo as Foo` to the
  relevant `__init__.py`. The redundant `as Foo` form matters — it is also
  required by pyright's strict-mode `reportPrivateUsage` rule.
- **Define in-file in a single-file module:** include `"Foo"` in the module's
  `__all__` (e.g., see `rerun_sdk/rerun/urdf.py`, `rerun_sdk/rerun/server.py`).
- **Stand up a Track A page for a brand-new subpackage:** add a row to
  `DOCUMENTED_PACKAGES` in `docs/gen_common_index.py` mapping the dotted path
  to its nav title (e.g., `"rerun.foo": ("Foo",)` for a top-level entry, or
  `"rerun.bar.baz": ("Bar", "Baz")` to nest it under "Bar"). The first build
  will tell you about every public symbol so you can decide what (if anything)
  belongs in `EXPLICIT_DOC_EXCLUDES`.
- **Group symbols on the landing page:** add to `CURATED_GROUPS` in
  `docs/gen_common_index.py`. Curated groups are tables only — they do not
  gate coverage and may safely duplicate symbols already listed by Track A.

### Hiding a public symbol from docs

Add it (per package) to `EXPLICIT_DOC_EXCLUDES` in `docs/gen_common_index.py`
with an inline comment explaining why. Each entry is a deliberate decision;
unexplained entries get rejected in code review.

### What the build validates

`pixi run py-docs-build` fails (and CI fails) on any of:

- A new top-level subpackage or module under `rerun_sdk/rerun/` — or a new
  nested subpackage (a directory with `__init__.py`) under any documented
  package — that is neither in `DOCUMENTED_PACKAGES` nor in
  `EXCLUDED_FROM_TRACK_A`. This is the freshness check that prevents new
  modules from going silently undocumented.
- A `DOCUMENTED_PACKAGES` or `EXCLUDED_FROM_TRACK_A` entry that no longer
  exists on disk (catches renames and removals).
- A `DOCUMENTED_PACKAGES` entry whose module has no public surface
  (no `__all__`, no redundant aliases, no in-file definitions).
- A `DOCUMENTED_PACKAGES` entry whose entire public surface is in
  `EXPLICIT_DOC_EXCLUDES` (the page would emit `members: []`, which is
  undefined in mkdocstrings).
- A `CURATED_GROUPS` entry that references a symbol that doesn't exist —
  catches stale entries when symbols are renamed or removed.

### Known limitations

- **PEP 562 `__getattr__` aliases** (used for deprecated re-exports) are
  invisible to static analysis. To document such a name, expose it via a
  real redundant-alias re-export and accept the validator's prompt.
- **Dynamic `__all__` constructions** (e.g., `__all__ = list(SOMETHING)`)
  are not supported; keep `__all__` a static list/tuple of string constants.

## Getting started with docs

### Serving the docs locally

This will watch the contents of the `rerun_py` folder and refresh documentation live as files are changed.

```sh
pixi run py-docs-serve
```

### How versioned docs are generated and served

Our documentation is versioned with releases and generated via [mkdocs](https://github.com/mkdocs/mkdocs).
The mkdocs dependencies are managed via uv (see the `docs` dependency group in `pyproject.toml`).

The documentation exists as bucket on GCS which is hosted on the <https://ref.rerun.io> domain.

Every commit that lands to main will generate bleeding edge documentation as HEAD. Behind the scenes, a
GitHub action is running `pixi run py-docs-build`, and uploading the result to GCS at
[`docs/python/main`](https://ref.rerun.io/docs/python/main).

Releases will push to a version instead: [`docs/python/0.23.3`](https://ref.rerun.io/docs/python/0.23.3)
