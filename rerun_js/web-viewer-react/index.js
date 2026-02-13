import React, { createRef } from "react";
import * as rerun from "@rerun-io/web-viewer";

// NOTE: We're intentionally not exposing `allow_fullscreen` and `enable_history`.
//       Those features are already pretty sensitive to the environment, especially
//       so in React where all normal behavior of web APIs goes out of the window.
/**
 * @typedef BaseProps
 * @property {string | string[]} rrd URL(s) of the `.rrd` file(s) to load.
 *                                   Changing this prop will open any new unique URLs as recordings,
 *                                   and close any URLs which are not present.
 * @property {string} [width] CSS width of the viewer's parent div
 * @property {string} [height] CSS height of the viewer's parent div
 *
 * @typedef {(
 *   Omit<import("@rerun-io/web-viewer").WebViewerOptions, "allow_fullscreen" | "enable_history">
 *   & BaseProps
 *   & import("./types.d.ts").ViewerEvents
 * )} Props
 */

/**
 * Wrapper for `WebViewer` from the `@rerun-io/web-viewer`.
 *
 * This component creates and manages the web viewer's `canvas` element.
 *
 * @extends {React.Component<Props>}
 */
export default class WebViewer extends React.Component {
  /** @type {React.RefObject<HTMLDivElement>} */
  #parent = /** @type {React.RefObject<HTMLDivElement>} */ (createRef());

  /** @type {rerun.WebViewer} */
  #handle;

  /** @param {Props} props */
  constructor(props) {
    super(props);

    this.#handle = new rerun.WebViewer();
  }

  componentDidMount() {
    startViewer(
      this.#handle,
      /** @type {HTMLDivElement} */ (this.#parent.current),
      () => this.props,
    );
  }

  componentDidUpdate(/** @type {Props} */ prevProps) {
    if (
      keysChanged(prevProps, this.props, [
        "hide_welcome_screen",
        "manifest_url",
        "render_backend",
      ])
    ) {
      // We have to restart the viewer, because the above
      // props are _startup_ options only, and we don't
      // want to break that promise by setting them
      // after the viewer has been started.
      this.#handle.stop();

      this.#handle = new rerun.WebViewer();
      startViewer(
        this.#handle,
        /** @type {HTMLDivElement} */ (this.#parent.current),
        () => this.props,
      );
    } else {
      // We only need to diff the recordings.

      const prev = toArray(prevProps.rrd);
      const current = toArray(this.props.rrd);
      const { added, removed } = diff(prev, current);
      this.#handle.open(added);
      this.#handle.close(removed);
    }
  }

  componentWillUnmount() {
    this.#handle.stop();
  }

  render() {
    const { width = "640px", height = "360px" } = this.props;
    return React.createElement("div", {
      className: "rerun-web-viewer",
      style: { width, height, position: "relative" },
      ref: this.#parent,
    });
  }
}

/** @param {string} str */
function pascalToSnake(str) {
  let out = "";
  for (let i = 0; i < str.length; i++) {
    const code = str.charCodeAt(i);

    // A–Z ?
    if (code >= 65 && code <= 90) {
      // if not first char, prepend underscore
      if (i > 0) out += "_";
      // convert to lowercase by adding 32
      out += String.fromCharCode(code + 32);
    } else {
      // everything else (a–z, 0–9, etc.) goes straight through
      out += String.fromCharCode(code);
    }
  }
  return out;
}

/**
 * @param {rerun.WebViewer} handle
 * @param {HTMLElement} parent
 * @param {() => Props} getProps
 */
function startViewer(handle, parent, getProps) {
  const props = getProps();
  handle.start(toArray(props.rrd), parent, {
    manifest_url: props.manifest_url,
    render_backend: props.render_backend,
    hide_welcome_screen: props.hide_welcome_screen,
    theme: props.theme,

    // NOTE: `width`, `height` intentionally ignored, they will
    //       instead be used on the parent `div` element
    width: "100%",
    height: "100%",
  });

  for (const key of Object.keys(props)) {
    if (key.startsWith("on")) {
      /** @type {any} */
      const event = pascalToSnake(key.slice(2));
      /** @type {any} */
      const callback = /** @type {any} */ (getProps())[key];
      handle.on(event, callback);
    }
  }
}

/**
 * Return the difference between the two arrays.
 *
 * @param {string[]} prev
 * @param {string[]} current
 * @returns {{ added: string[], removed: string[] }}
 */
function diff(prev, current) {
  const prevSet = new Set(prev);
  const currentSet = new Set(current);
  return {
    added: current.filter((v) => !prevSet.has(v)),
    removed: prev.filter((v) => !currentSet.has(v)),
  };
}

/**
 * Returns `true` if any of the `keys` changed between `prev` and `curr`.
 *
 * The definition of "changed" is any of removed, added, value changed.
 *
 * @template {Record<string, any>} T
 * @param {T} prev
 * @param {T} curr
 * @param {(keyof T)[]} keys
 */
function keysChanged(prev, curr, keys) {
  for (const key of keys) {
    if (prev[key] !== curr[key]) {
      return false;
    }
  }
  return true;
}

/**
 * @template T
 * @param {T | T[]} a
 * @returns {T[]}
 */
function toArray(a) {
  return Array.isArray(a) ? a : [a];
}
