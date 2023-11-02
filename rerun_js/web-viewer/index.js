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

  /**
   * @param {string} url
   * @param {HTMLElement} [parent]
   * @returns {Promise<this>}
   */
  async start(url, parent = document.body) {
    const canvas = document.createElement("canvas");
    canvas.id = randomId();
    parent.append(canvas);

    let WebHandle_class = await load();
    const handle = new WebHandle_class();
    await handle.start(canvas.id, url);
    if (handle.has_panicked()) {
      throw new Error(`Web viewer crashed: ${handle.panic_message()}`);
    }

    this.#canvas = canvas;
    this.#handle = handle;

    return this;
  }

  stop() {
    const canvas = this.#canvas;
    this.#canvas = null;
    if (canvas) {
      canvas.remove();
    }

    const handle = this.#handle;
    this.#handle = null;
    if (handle) {
      handle.destroy();
      handle.free();
    }
  }
}

