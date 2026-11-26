import { expect, test as base, type Page } from "@playwright/test";

// Each `WebViewer` must run in its own Wasm instance, and release that instance's memory when it stops.
// Both depend on how `rerun_js/web-viewer/build-wasm.mjs` patches the output of `wasm-bindgen`.

declare global {
  interface Window {
    /** The memory of every Wasm instance the page has created. */
    wasmMemories: WeakRef<WebAssembly.Memory>[];

    /** The buffers that the memories of stopped viewers had when they stopped. */
    stoppedWasmBuffers: WeakRef<ArrayBufferLike>[];

    /** Every event listener the page has added, so that none of them is ever collected. */
    leakedListeners: unknown[];
  }
}

/** A page with an empty viewer in `window.viewer`, that tracks its Wasm memories and leaks its closures. */
const test = base.extend<{ viewer: Page }>({
  viewer: async ({ page }, use) => {
    const errors: string[] = [];
    page.on("pageerror", (err) => errors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    await page.addInitScript(() => {
      window.wasmMemories = [];
      window.stoppedWasmBuffers = [];
      window.leakedListeners = [];

      const instantiate = WebAssembly.instantiate.bind(WebAssembly) as (
        ...args: unknown[]
      ) => Promise<WebAssembly.Instance | WebAssembly.WebAssemblyInstantiatedSource>;
      WebAssembly.instantiate = (async (...args: unknown[]) => {
        const result = await instantiate(...args);
        const instance = result instanceof WebAssembly.Instance ? result : result.instance;
        if (instance.exports.memory instanceof WebAssembly.Memory) {
          window.wasmMemories.push(new WeakRef(instance.exports.memory));
        }
        return result;
      }) as typeof WebAssembly.instantiate;

      // The viewer's listeners are closures of its Wasm instance.
      // A page can easily hold on to one of them, and that must not keep the memory alive.
      const addEventListener = EventTarget.prototype.addEventListener;
      EventTarget.prototype.addEventListener = function (...args) {
        window.leakedListeners.push(args[1]);
        addEventListener.apply(this, args);
      };
    });

    await page.goto("/?url=");
    // Same as in `fixtures.ts`: loading the Wasm takes a while on a software rasterizer.
    await expect
      .poll(() => page.evaluate(() => window.viewer?.ready === true), { timeout: 120_000 })
      .toBe(true);

    await use(page);

    expect(errors).toEqual([]);
  },
});

/** Start one more viewer, next to those that already run. */
async function startViewer(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const parent = document.body.appendChild(document.createElement("div"));
    await new window.rerun.WebViewer().start(null, parent, { hideWelcomeScreen: true });
  });
}

/** Stop `window.viewer`, and remember the buffer of every memory that is alive at that point. */
async function stopViewer(page: Page): Promise<void> {
  await page.evaluate(() => {
    // Hold the memories for the rest of this task, so that the buffers can be read after the stop.
    const memories = window.wasmMemories.map((memory) => memory.deref());
    window.viewer.stop();
    for (const memory of memories) {
      if (memory) {
        window.stoppedWasmBuffers.push(new WeakRef(memory.buffer));
      }
    }
  });
}

test("each viewer has its own Wasm memory", async ({ viewer }) => {
  await startViewer(viewer);

  const memories = await viewer.evaluate(() => {
    const memories = window.wasmMemories.map((memory) => memory.deref());
    return {
      count: memories.length,
      alive: memories.filter((memory) => memory !== undefined).length,
      distinct: new Set(memories).size,
    };
  });
  expect(memories).toEqual({ count: 2, alive: 2, distinct: 2 });
});

test("a stopped viewer releases its Wasm memory, even if the page leaks its closures", async ({
  viewer,
}) => {
  // Stop and start a few times, like a notebook cell that runs again and again.
  for (let round = 0; round < 3; round++) {
    await stopViewer(viewer);
    await viewer.evaluate(() =>
      window.viewer.start(null, document.getElementById("viewer")!, { hideWelcomeScreen: true }),
    );
  }
  await stopViewer(viewer);
  expect(await viewer.evaluate(() => window.wasmMemories.length)).toBe(4);
  // `wasm-bindgen` marks its closures with `_wbg_cb_unref`.
  expect(
    await viewer.evaluate(() =>
      window.leakedListeners.some(
        (listener) => typeof listener === "function" && "_wbg_cb_unref" in listener,
      ),
    ),
  ).toBe(true);

  // One collection is not always enough, and what is still pending after 30 seconds is a leak.
  await expect
    .poll(
      async () => {
        await viewer.requestGC();
        return viewer.evaluate(() => ({
          memories: window.wasmMemories.filter((memory) => memory.deref() !== undefined).length,
          buffers: window.stoppedWasmBuffers.filter((buffer) => buffer.deref() !== undefined)
            .length,
        }));
      },
      { timeout: 30_000 },
    )
    .toEqual({ memories: 0, buffers: 0 });
});
