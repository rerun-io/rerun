import React, { createRef } from "react";
import * as rerun from "@rerun-io/web-viewer";

/**
 * @typedef Props
 * @property {string | string[]} rrd URL(s) of the `.rrd` file(s) to load.
 *                                   Changing this prop will open any new unique URLs as recordings,
 *                                   and close any URLs which are not present.
 * @property {string} [width] CSS width of the viewer's parent div
 * @property {string} [height] CSS height of the viewer's parent div
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
  #parent = createRef();

  /** @type {rerun.WebViewer} */
  #handle;

  /** @type {string[]} */
  #recordings = [];

  /** @param {Props} props */
  constructor(props) {
    super(props);

    this.#handle = new rerun.WebViewer();
    this.#recordings = toArray(props.rrd);
  }

  componentDidMount() {
    const current = /** @type {HTMLDivElement} */ (this.#parent.current);
    this.#handle.start(this.#recordings, current);
  }

  componentDidUpdate(/** @type {Props} */ prevProps) {
    const prev = toArray(prevProps.rrd);
    const current = toArray(this.props.rrd);
    // Diff recordings when `rrd` prop changes.
    const { added, removed } = diff(prev, current);
    this.#handle.open(added);
    this.#handle.close(removed);
  }

  componentWillUnmount() {
    this.#handle.stop();
  }

  render() {
    const { width = "100%", height = "640px" } = this.props;
    return React.createElement("div", {
      className: "rerun-web-viewer",
      style: { width, height, position: "relative" },
      ref: this.#parent,
    });
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
 * @template T
 * @param {T | T[]} a
 * @returns {T[]}
 */
function toArray(a) {
  return Array.isArray(a) ? a : [a];
}
