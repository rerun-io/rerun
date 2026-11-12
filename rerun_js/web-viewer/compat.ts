/** Accepts the deprecated snake_case spellings of viewer API options, and adds them as aliases on returned objects. */
const FIELD_NAMES = {
  "active_store_id": "activeStoreId",
  "allow_blocking_native_modal": "allowBlockingNativeModal",
  "allow_fullscreen": "allowFullscreen",
  "application_id": "applicationId",
  "asset_id": "assetId",
  "asset_ids": "assetIds",
  "base_url": "baseUrl",
  "blocking_native_modal": "blockingNativeModal",
  "blueprint_id": "blueprintId",
  "catalog_url": "catalogUrl",
  "check_for_updates_on_startup": "checkForUpdatesOnStartup",
  "component_type": "componentType",
  "container_id": "containerId",
  "container_name": "containerName",
  "current_time": "currentTime",
  "dataset_url": "datasetUrl",
  "display_name": "displayName",
  "duration_nanos": "durationNanos",
  "edge_filter": "edgeFilter",
  "enable_history": "enableHistory",
  "entity_path": "entityPath",
  "fallback_token": "fallbackToken",
  "frame_filter": "frameFilter",
  "has_static": "hasStatic",
  "hide_welcome_screen": "hideWelcomeScreen",
  "include_transforms": "includeTransforms",
  "include_unavailable": "includeUnavailable",
  "instance_id": "instanceId",
  "manifest_url": "manifestUrl",
  "max_entities": "maxEntities",
  "max_size": "maxSize",
  "memory_limit": "memoryLimit",
  "omitted_entities": "omittedEntities",
  "panel_state_overrides": "panelStateOverrides",
  "partition_id": "segmentId",
  "paths_only": "pathsOnly",
  "play_state": "playState",
  "recording_id": "recordingId",
  "render_backend": "renderBackend",
  "segment_id": "segmentId",
  "server_origin": "serverOrigin",
  "signed_in_url": "signedInUrl",
  "signed_out_url": "signedOutUrl",
  "store_id": "storeId",
  "store_ids": "storeIds",
  "table_url": "tableUrl",
  "time_range": "timeRange",
  "time_type": "timeType",
  "timestamp_nanos": "timestampNanos",
  "video_decoder": "videoDecoder",
  "view_id": "viewId",
  "view_name": "viewName",
  "viewer_base_url": "viewerBaseUrl",
  "viewer_version": "viewerVersion",
} as const;

type FieldNames = typeof FIELD_NAMES;

/** Only add an alias when its camelCase field exists on the object. */
type LegacyAliases<T> = {
  [K in keyof FieldNames as FieldNames[K] extends keyof T
    ? K
    : never]: FieldNames[K] extends keyof T ? T[FieldNames[K]] : never;
};

type LegacyFields<T> = T extends readonly (infer Item)[]
  ? LegacyFields<Item>[]
  : T extends object
    ? {
        [K in keyof T]: K extends (typeof NESTED_FIELDS)[number] ? LegacyFields<T[K]> : T[K];
      } & LegacyAliases<T>
    : T;

// Recurse only into known API containers; arbitrary user data must keep its keys.
const NESTED_FIELDS = [
  "components",
  "currentTime",
  "entities",
  "entries",
  "items",
  "loading",
  "login",
  "recordings",
  "reports",
  "servers",
  "timeRange",
  "timelines",
  "views",
] as const;
const nestedFields = new Set<string>(NESTED_FIELDS);

/** Normalize API option names without traversing arbitrary blueprint or graph data. */
export function normalizeOptions<T>(value: T): T {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return value;

  const result = { ...value } as Record<string, unknown>;
  for (const [legacy, canonical] of Object.entries(FIELD_NAMES)) {
    // A defined camelCase value wins, including false, zero, and empty strings.
    if (result[canonical] === undefined && legacy in result) {
      result[canonical] = result[legacy];
    }
    delete result[legacy];
  }

  for (const key of nestedFields) {
    if (key in result) {
      const child = result[key];
      result[key] = Array.isArray(child) ? child.map(normalizeOptions) : normalizeOptions(child);
    }
  }
  return result as T;
}

/** Both spellings share a value, including after assignment; only the camelCase one is enumerable. */
export function withLegacyFields<T>(value: T): LegacyFields<T> {
  if (Array.isArray(value)) return value.map(withLegacyFields) as LegacyFields<T>;
  if (value === null || typeof value !== "object") return value as LegacyFields<T>;

  const result = value as Record<string, unknown>;
  for (const key of nestedFields) {
    if (key in result) {
      result[key] = withLegacyFields(result[key]);
    }
  }

  for (const [legacy, canonical] of Object.entries(FIELD_NAMES)) {
    if (canonical in result) {
      // Accessors keep both spellings in sync when callers assign through either one.
      Object.defineProperty(result, legacy, {
        enumerable: false,
        configurable: true,
        get: () => result[canonical],
        set: (value) => {
          result[canonical] = value;
        },
      });
    }
  }
  return value as LegacyFields<T>;
}
