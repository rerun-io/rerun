# Hypothesis 2: Reduce catch_and_log_exceptions overhead

## Hypothesis
`catch_and_log_exceptions` is called 4x per log call. Each `__enter__`/`__exit__` does multiple `getattr()` on `threading.local()`, which goes through the threading.local descriptor protocol. Replacing these with direct `__dict__` access on the thread-local should be faster.

## Rationale
`getattr(_rerun_exception_ctx, "attr", default)` goes through `threading.local.__getattribute__` which does thread ID lookup + dict lookup + default handling. `_rerun_exception_ctx.__dict__` returns the thread-local dict directly; subsequent `.get()` calls are plain dict operations.

## Code Changes
In `error_utils.py`:
1. Replaced all `getattr(_rerun_exception_ctx, "attr", default)` with `ctx = _rerun_exception_ctx.__dict__; ctx.get("attr", default)`
2. Replaced all `_rerun_exception_ctx.attr = value` with `ctx["attr"] = value`
3. Applied same pattern to `strict_mode()` function and `_send_warning_or_raise()`

## Results (release build, 100 entities x 1000 time steps)

| Metric | Baseline | H2 | Change |
|--------|----------|-----|--------|
| Full log | 36.8k xforms/s | 40.5k xforms/s | +10% |
| Create only | 112k xforms/s | 119k xforms/s | +6% |

## Notes
Also attempted a "fast-path" approach that skips all thread-local bookkeeping when not in strict mode (~14% improvement). However, this breaks the depth-tracking needed for correct warning batching in nested handlers. The failing test (`test_stack_tracking`) verifies that warnings point to the outermost user call-site, which requires all handlers to participate in depth counting.

## Decision: KEEP
10% improvement with zero behavior change.
