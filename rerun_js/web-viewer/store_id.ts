/** The parts of a store id, as {@link parse_store_id} splits it. */
export interface StoreIdParts {
  /** What the store holds: `Recording` or `Blueprint`. */
  kind: string;

  application_id: string;
  recording_id: string;
}

/**
 * Split a store id, `{kind}:{application_id}:{recording_id}`, into its parts.
 *
 * The application id escapes `:` as `\:` and `\` as `\\`, which this undoes.
 * The recording id is the rest, colons and all.
 */
export function parse_store_id(store_id: string): StoreIdParts {
  const kind_end = store_id.indexOf(":");
  let application_id = "";
  let i = kind_end + 1;
  while (i < store_id.length && store_id[i] !== ":") {
    if (store_id[i] === "\\" && i + 1 < store_id.length) {
      i += 1;
    }
    application_id += store_id[i];
    i += 1;
  }
  return {
    kind: kind_end < 0 ? "" : store_id.slice(0, kind_end),
    application_id,
    recording_id: store_id.slice(i + 1),
  };
}
