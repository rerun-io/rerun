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
 * @property {boolean} [follow_if_http] Whether to open HTTP `.rrd` sources in following mode.
 *                                      Defaults to `false`. Ignored for non-HTTP sources.
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

      if (!this.#handle.ready) {
        return;
      }

      syncRecordings(
        this.#handle,
        toArray(prevProps.rrd),
        toArray(this.props.rrd),
        prevProps.follow_if_http,
        this.props.follow_if_http,
      );
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
  const initial = toArray(props.rrd);
  const initialFollowIfHttp = props.follow_if_http;
  handle
    .start(
      initial,
      parent,
      {
        manifest_url: props.manifest_url,
        render_backend: props.render_backend,
        hide_welcome_screen: props.hide_welcome_screen,
        theme: props.theme,

        // NOTE: `width`, `height` intentionally ignored, they will
        //       instead be used on the parent `div` element
        width: "100%",
        height: "100%",
      },
      {
        follow_if_http: initialFollowIfHttp,
      },
    )
    .then(() => {
      if (!handle.ready) {
        return;
      }

      const { rrd, follow_if_http } = getProps();
      syncRecordings(
        handle,
        initial,
        toArray(rrd),
        initialFollowIfHttp,
        follow_if_http,
      );
    })
    .catch(() => {});

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
 * Reconcile the currently open recordings with the latest props.
 *
 * @param {rerun.WebViewer} handle
 * @param {string[]} prev
 * @param {string[]} current
 * @param {boolean | undefined} prevFollowIfHttp
 * @param {boolean | undefined} followIfHttp
 */
function syncRecordings(handle, prev, current, prevFollowIfHttp, followIfHttp) {
  const { added, removed } = diff(prev, current);
  const reopened =
    prevFollowIfHttp !== followIfHttp
      ? intersection(prev, current).filter(isHttpSource)
      : [];

  if (removed.length > 0 || reopened.length > 0) {
    handle.close([...removed, ...reopened]);
  }
  if (added.length > 0) {
    handle.open(added, { follow_if_http: followIfHttp });
  }
  if (reopened.length > 0) {
    handle.open(reopened, { follow_if_http: followIfHttp });
  }
}

/**
 * Return the values present in both arrays.
 *
 * @param {string[]} prev
 * @param {string[]} current
 * @returns {string[]}
 */
function intersection(prev, current) {
  const prevSet = new Set(prev);
  return current.filter((v) => prevSet.has(v));
}

/**
 * Returns `true` if the recording source is affected by `follow_if_http`.
 *
 * @param {string} url
 */
function isHttpSource(url) {
  try {
    const protocol = new URL(url, document.baseURI).protocol;
    return protocol === "http:" || protocol === "https:";
  } catch {
    return false;
  }
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
      return true;
    }
  }
  return false;
}

/**
 * @template T
 * @param {T | T[] | undefined | null} a
 * @returns {T[]}
 */
function toArray(a) {
  if (a == null) return [];
  return Array.isArray(a) ? a : [a];
}
