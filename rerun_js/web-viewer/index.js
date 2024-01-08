// @ts-check

/** @type {(typeof import("./re_viewer.js").WebHandle) | null} */
let WebHandle = null;

/** @returns {Promise<(typeof import("./re_viewer.js").WebHandle)>} */
async function load() {
  if (WebHandle) {
    return WebHandle;
  }
  WebHandle = (await import("./re_viewer.js")).WebHandle;
  return WebHandle;
}

/** @returns {string} */
function randomId() {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes)
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export class WebViewer {
  /** @type {(import("./re_viewer.js").WebHandle) | null} */
  #handle = null;

  /** @type {HTMLCanvasElement | null} */
  #canvas = null;

  /** @type {'ready' | 'starting' | 'stopped'} */
  #state = "stopped";

  /**
   * Start the viewer.
   *
   * @param {string | string[]} [rrd] URLs to `.rrd` files or WebSocket connections to our SDK.
   * @param {HTMLElement} [parent] The element to attach the canvas onto.
   * @returns {Promise<void>}
   */
  async start(rrd, parent = document.body) {
    if (this.#state !== "stopped") return;
    this.#state = "starting";

    this.#canvas = document.createElement("canvas");
    this.#canvas.id = randomId();
    parent.append(this.#canvas);

    let WebHandle_class = await load();
    if (this.#state !== "starting") return;

    this.#handle = new WebHandle_class();
    await this.#handle.start(this.#canvas.id, undefined);
    if (this.#state !== "starting") return;

    if (this.#handle.has_panicked()) {
      throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
    }

    if (rrd) {
      this.open(rrd);
    }

    console.log("started");

    return;
  }

  /**
   * Returns `true` if the viewer is ready for
   */
  get ready() {
    return this.#state === "ready";
  }

  /**
   * Open a recording.
   *
   * The viewer must have been started via `WebViewer.start`.
   *
   * @see {WebViewer.start}
   *
   * @param {string | string[]} rrd URLs to `.rrd` files or WebSocket connections to our SDK.
   */
  open(rrd) {
    if (!this.#handle) {
      throw new Error(`attempted to open \`${rrd}\` in a stopped viewer`);
    }
    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      this.#handle.add_receiver(url);
      if (this.#handle.has_panicked()) {
        throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
      }
    }
  }

  /**
   * Close a recording.
   *
   * The viewer must have been started via `WebViewer.start`.
   *
   * @see {WebViewer.start}
   *
   * @param {string | string[]} rrd URLs to `.rrd` files or WebSocket connections to our SDK.
   */
  close(rrd) {
    if (!this.#handle) {
      throw new Error(`attempted to close \`${rrd}\` in a stopped viewer`);
    }
    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      this.#handle.remove_receiver(url);
      if (this.#handle.has_panicked()) {
        throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
      }
    }
  }

  /**
   * Stop the viewer, freeing all associated memory.
   *
   * The same viewer instance may be started multiple times.
   */
  stop() {
    if (this.#state !== "stopped") return;
    this.#state = "stopped";

    this.#canvas?.remove();
    this.#handle?.destroy();
    this.#handle?.free();

    this.#canvas = null;
    this.#handle = null;

    console.log("stopped");
  }
}
