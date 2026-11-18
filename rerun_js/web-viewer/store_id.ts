import { withLegacyFields } from "./compat.js";
/**
 * Names a recording or blueprint store in the viewer: `{kind}:{application_id}:{recording_id}`,
 * e.g. `Recording:my_app:1830B33B45B963E7`.
 *
 * Pass it back whole wherever the API takes one. It is not the bare recording id at its end.
 */
export type StoreId = RecordingStoreId | BlueprintStoreId;

/** The {@link StoreId} of a recording. */
export type RecordingStoreId = `Recording:${string}:${string}`;

/** The {@link StoreId} of a blueprint. */
export type BlueprintStoreId = `Blueprint:${string}:${string}`;

/** The parts of a store id, as {@link parseStoreId} splits it. */
export interface StoreIdParts {
  /** What the store holds: `Recording` or `Blueprint`. */
  kind: string;

  applicationId: string;
  /** @deprecated Since 0.43.0. Use `applicationId` instead. */
  "application_id": string;
  recordingId: string;
  /** @deprecated Since 0.43.0. Use `recordingId` instead. */
  "recording_id": string;
}

/**
 * Split a store id, `{kind}:{application_id}:{recording_id}`, into its parts.
 *
 * The application id escapes `:` as `\:` and `\` as `\\`, which this undoes.
 * The recording id is the rest, colons and all.
 */
export function parseStoreId(storeId: string): StoreIdParts {
  const kindEnd = storeId.indexOf(":");
  let applicationId = "";
  let i = kindEnd + 1;
  while (i < storeId.length && storeId[i] !== ":") {
    if (storeId[i] === "\\" && i + 1 < storeId.length) {
      i += 1;
    }
    applicationId += storeId[i];
    i += 1;
  }
  return withLegacyFields({
    kind: kindEnd < 0 ? "" : storeId.slice(0, kindEnd),
    applicationId,
    recordingId: storeId.slice(i + 1),
  });
}

/** @deprecated Since 0.43.0. Use {@link parseStoreId} instead. */
// oxlint-disable-next-line id-match -- Deprecated public export.
export const parse_store_id = parseStoreId;
