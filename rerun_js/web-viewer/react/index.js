import React, { useEffect, useRef } from "react";
import * as rerun from "@rerun-io/web-viewer";

/** @typedef {import("react")} React */

/**
 * @typedef Props
 * @property {string} rrd URL of the `.rrd` file to load
 * @property {string} [width] CSS width of the viewer's parent div
 * @property {string} [height] CSS height of the viewer's parent div
 */

/**
 * Wrapper for `WebViewer` from the `@rerun-io/web-viewer`.
 *
 * This component creates and manages the web viewer's `canvas` element.
 *
 * The web viewer is restarted each time `rrd` changes.
 * Starting the web viewer is an expensive operation, so be careful with changing it too often!
 *
 * @param {Props} props
 */
export default function WebViewer(props) {
  const { width = "100%", height = "640px", rrd } = props;

  /**
   * Parent DOM node
   * @type {React.RefObject<HTMLDivElement>}
   */
  const parent = useRef(null);
  /**
   * Web viewer instance
   * @type {React.MutableRefObject<rerun.WebViewer | undefined>}
   */
  const viewer = useRef();

  useEffect(
    () => {
      if (parent.current) {
        // Start the web viewer when the parent div is mounted to the DOM.
        const w = new rerun.WebViewer();
        w.start(rrd, parent.current);
        viewer.current = w;
        return () => {
          // Stop the web viewer when the component is unmounted.
          w.stop();
          viewer.current = undefined;
        };
      }
    },
    // The web viewer will be restarted when:
    // - `parent` is added/moved/removed in the DOM
    // - `rrd` changes
    [parent.current, rrd],
  );

  return React.createElement("div", {
    className: "rerun-web-viewer",
    style: { width, height, position: "relative" },
    ref: parent,
  });
}
