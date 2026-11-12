import { normalizeOptions, withLegacyFields } from "./compat.js";
// @ts-ignore
import type { WebHandle, wasm_bindgen as wasmBindgen } from "./re_viewer.js";
import { create, fromBinary, toBinary, type MessageInitShape } from "@bufbuild/protobuf";
import {
  GetTransformGraphRequest_EdgeFilter as GetTransformGraphRequestEdgeFilter,
  GetTransformGraphRequest_FrameFilter as GetTransformGraphRequestFrameFilter,
  GetStateResponseSchema,
  ViewerControlRequestSchema,
  ViewerControlResponseSchema,
  type GetSelectionResponse,
  type GetStateResponse,
  PlayState as ProtoPlayState,
  type RecordingEntitySchema,
  type ViewerCommand as ProtoViewerCommand,
  type ViewerControlRequest,
  type ViewerControlResponse,
  type ViewerTimeline as ProtoViewerTimeline,
} from "./proto/rerun/v1alpha1/viewer_control_pb.js";
import { EntryKind as ProtoEntryKind } from "./proto/rerun/v1alpha1/cloud_pb.js";
import { TimeType as ProtoTimeType } from "./proto/rerun/v1alpha1/common_pb.js";
import { parseExactJson, stringifyExactJson } from "./exact_json.js";
import {
  type BlueprintStoreId,
  parseStoreId,
  type RecordingStoreId,
  type StoreId,
} from "./store_id.js";

export {
  parseStoreId,
  // oxlint-disable-next-line id-match, typescript/no-deprecated -- Deprecated public export.
  parse_store_id,
  type BlueprintStoreId,
  type RecordingStoreId,
  type StoreId,
  type StoreIdParts,
} from "./store_id.js";

/** A UUID in the hyphenated form, e.g. `67e55044-10b1-426f-9247-bb680e5fe0c8`. */
export type Uuid = string;

/** What the viewer shows, as {@link WebViewer.getState} reports it. */
export interface ViewerState {
  /** Where the viewer is, for {@link WebViewer.setLocation}. Empty if that has no location. */
  location: string;

  /** The active recording. */
  activeStoreId?: RecordingStoreId;
  /** @deprecated Since 0.43.0. Use `activeStoreId` instead. */
  "active_store_id"?: RecordingStoreId;

  /** Every recording open in the viewer. */
  recordings: ViewerRecording[];

  /** The views of the active recording's blueprint. */
  views: ViewerView[];

  /** What the viewer is still loading, empty once everything has arrived. */
  loading: ViewerLoadingSource[];

  /** The servers in the viewer's left panel. */
  servers: ViewerServer[];

  /** The origin of the catalog server the viewer hosts, if any. */
  catalogUrl?: string;
  /** @deprecated Since 0.43.0. Use `catalogUrl` instead. */
  "catalog_url"?: string;

  /** The version of the viewer. */
  viewerVersion?: string;
  /** @deprecated Since 0.43.0. Use `viewerVersion` instead. */
  "viewer_version"?: string;
}

/** A recording open in the viewer. */
export interface ViewerRecording {
  /** The recording's store id. */
  storeId: RecordingStoreId;
  /** @deprecated Since 0.43.0. Use `storeId` instead. */
  "store_id": RecordingStoreId;

  /** The location that shows this recording, for {@link WebViewer.setLocation}. */
  location: string;

  /** The recording's timelines. */
  timelines: ViewerTimeline[];

  /** The recording's time cursor, if it has one. */
  currentTime?: TimeCursor;
  /** @deprecated Since 0.43.0. Use `currentTime` instead. */
  "current_time"?: TimeCursor;

  /** Whether the time cursor is paused, playing, or following the latest data. */
  playState?: PlayState;
  /** @deprecated Since 0.43.0. Use `playState` instead. */
  "play_state"?: PlayState;
}

/** A timeline of a recording. */
export interface ViewerTimeline {
  /** The timeline's name. */
  name: string;

  /** What kind of time the timeline holds. */
  timeType?: TimeType;
  /** @deprecated Since 0.43.0. Use `timeType` instead. */
  "time_type"?: TimeType;

  /** The first and last time on the timeline, if it has any data. */
  timeRange?: TimeRange;
  /** @deprecated Since 0.43.0. Use `timeRange` instead. */
  "time_range"?: TimeRange;

  /**
   * Where the time cursor is on this timeline, in its own unit.
   *
   * Each timeline keeps its own position, so this is also set for timelines other than the active one.
   * Absent if the cursor has never been on it.
   */
  currentTime?: bigint;
  /** @deprecated Since 0.43.0. Use `currentTime` instead. */
  "current_time"?: bigint;
}

/** Where a recording's time cursor is. */
export interface TimeCursor {
  /** The timeline the cursor is on. */
  timeline: string;

  /** What kind of time the timeline holds. */
  timeType?: TimeType;
  /** @deprecated Since 0.43.0. Use `timeType` instead. */
  "time_type"?: TimeType;

  /** The cursor's time, in the timeline's own unit, if it is set. */
  time?: bigint;
}

/** A range of time, in the timeline's own unit, both ends inclusive. */
export interface TimeRange {
  start: bigint;
  end: bigint;
}

/**
 * What kind of time a timeline holds, and so the unit of its times:
 * a sequence index, a duration in nanoseconds, or a timestamp in nanoseconds since the Unix epoch.
 */
export type TimeType = "sequence" | "duration" | "timestamp";

/** Whether a time cursor is paused, playing, or following the latest data as it arrives. */
export type PlayState = "paused" | "playing" | "following";

/** A view of the current blueprint. */
export interface ViewerView {
  viewId: Uuid;
  /** @deprecated Since 0.43.0. Use `viewId` instead. */
  "view_id": Uuid;

  /** The view's class, e.g. `Spatial3D`. */
  class: string;

  name: string;

  /** The view's origin entity path. */
  origin: string;

  visible: boolean;

  /** The warnings and errors the view reported the last time it was shown. */
  reports: ViewerReport[];
}

/** A warning or an error a view reported. */
export interface ViewerReport {
  /** `warning` or `error`. */
  severity: string;

  summary: string;
  details?: string;
}

/** A server in the viewer's left panel. */
export interface ViewerServer {
  origin: string;

  /** The server's datasets and tables. */
  entries: ViewerServerEntry[];

  /** Why the viewer could not list the server's entries, if it could not. */
  error?: string;
}

/** A dataset or table on a server. */
export interface ViewerServerEntry {
  name: string;
  kind?: EntryKind;

  /** The entry's URL, for {@link WebViewer.open} or {@link WebViewer.setLocation}. */
  url: string;
}

/** What kind of entry a server holds. */
export type EntryKind =
  | "dataset"
  | "dataset_view"
  | "table"
  | "table_view"
  | "blueprint_dataset"
  | "asset_dataset";

/** Something the viewer is still loading. */
export interface ViewerLoadingSource {
  /** What is being loaded: a file path, a URL's display name, or a segment id. */
  name: string;

  /** The same thing the viewer's own loading screen says. */
  status: string;
}

/** What the user has selected in the viewer, as {@link WebViewer.getSelection} reports it. */
export interface Selection {
  /** The recording that the selected entities and components belong to: the active recording. */
  storeId?: StoreId;

  /** The selected items, in selection order. Empty if nothing is selected. */
  items: SelectedItem[];
}

/** One selected item. */
export type SelectedItem =
  | {
      type: "entity";
      entityPath: string;

      /** The index of the selected instance, e.g. one point of a point cloud. Absent when the whole entity is selected. */
      instance?: bigint;

      /** The view it was selected in, as a `viewId` from {@link WebViewer.getState}. Absent when selected outside of a view. */
      viewId?: Uuid;
    }
  | {
      type: "component";
      entityPath: string;

      /** The component, e.g. `Points3D:positions`. */
      component: string;
    }
  | {
      type: "view";
      viewId: Uuid;
    }
  | {
      type: "container";
      containerId: Uuid;
    }
  | {
      type: "recording";
      storeId: StoreId;
    };

/** The public {@link Selection} of a `GetSelection` response, leaving out kinds this version does not know. */
function selectionOf(selection: GetSelectionResponse): Selection {
  const items: SelectedItem[] = [];
  for (const { kind } of selection.items) {
    switch (kind.case) {
      case "entity":
        items.push({
          type: "entity",
          entityPath: kind.value.entityPath,
          instance: kind.value.instance,
          viewId: kind.value.viewId?.uuid,
        });
        break;
      case "component":
        items.push({
          type: "component",
          entityPath: kind.value.entityPath,
          component: kind.value.component,
        });
        break;
      case "view":
        items.push({ type: "view", viewId: kind.value.viewId?.uuid ?? "" });
        break;
      case "container":
        items.push({ type: "container", containerId: kind.value.containerId?.uuid ?? "" });
        break;
      case "recording":
        items.push({ type: "recording", storeId: kind.value.storeId as StoreId });
        break;
    }
  }
  return { storeId: selection.storeId as StoreId | undefined, items };
}

/** The public {@link ViewerState} of a `GetState` response. */
function viewerStateOf(state: GetStateResponse): ViewerState {
  return withLegacyFields({
    location: state.location,
    activeStoreId: state.activeStoreId as RecordingStoreId | undefined,
    recordings: state.recordings.map((recording) => ({
      storeId: recording.storeId as RecordingStoreId,
      location: recording.location,
      timelines: recording.timelines.map(timelineOf),
      playState: playStateOf(recording.playState),
      currentTime: recording.currentTime && {
        timeline: recording.currentTime.timeline?.name ?? "",
        timeType: timeTypeOf(recording.currentTime.timeType),
        time: recording.currentTime.time?.time,
      },
    })),
    views: state.views.map((view) => ({
      viewId: view.viewId,
      class: view.class,
      name: view.name,
      origin: view.origin,
      visible: view.visible,
      reports: view.reports.map(({ severity, summary, details }) => ({
        severity,
        summary,
        details,
      })),
    })),
    loading: state.loading.map(({ name, status }) => ({ name, status })),
    servers: state.servers.map((server) => ({
      origin: server.origin,
      entries: server.entries.map((entry) => ({
        name: entry.name,
        kind: entryKindOf(entry.kind),
        url: entry.url,
      })),
      error: server.error,
    })),
    catalogUrl: state.catalogUrl,
    viewerVersion: state.viewerVersion,
  });
}

function playStateOf(playState: ProtoPlayState): PlayState | undefined {
  switch (playState) {
    case ProtoPlayState.PAUSED:
      return "paused";
    case ProtoPlayState.PLAYING:
      return "playing";
    case ProtoPlayState.FOLLOWING:
      return "following";
    default:
      return undefined;
  }
}

function playStateToProto(playState: PlayState | undefined): ProtoPlayState | undefined {
  switch (playState) {
    case "paused":
      return ProtoPlayState.PAUSED;
    case "playing":
      return ProtoPlayState.PLAYING;
    case "following":
      return ProtoPlayState.FOLLOWING;
    default:
      return undefined;
  }
}

function timeTypeOf(timeType: ProtoTimeType | undefined): TimeType | undefined {
  switch (timeType) {
    case ProtoTimeType.SEQUENCE:
      return "sequence";
    case ProtoTimeType.DURATION_NS:
      return "duration";
    case ProtoTimeType.TIMESTAMP_NS:
      return "timestamp";
    default:
      return undefined;
  }
}

function entryKindOf(kind: ProtoEntryKind): EntryKind | undefined {
  switch (kind) {
    case ProtoEntryKind.DATASET:
      return "dataset";
    case ProtoEntryKind.DATASET_VIEW:
      return "dataset_view";
    case ProtoEntryKind.TABLE:
      return "table";
    case ProtoEntryKind.TABLE_VIEW:
      return "table_view";
    case ProtoEntryKind.BLUEPRINT_DATASET:
      return "blueprint_dataset";
    case ProtoEntryKind.ASSET_DATASET:
      return "asset_dataset";
    default:
      return undefined;
  }
}

/** How severe a {@link ViewerLogEntry} is. */
export type ViewerLogLevel = "INFO" | "WARN" | "ERROR";

/** A log message the viewer emitted, as {@link WebViewer.getLogs} reports it. */
export interface ViewerLogEntry {
  /** Increases by one per message. Pass the last one seen to {@link WebViewer.getLogs} for only what is new. */
  sequence: bigint;

  /** How severe the message is. */
  level: ViewerLogLevel;

  /** The module that logged the message, starting with the crate name. */
  target: string;

  /** The log message. */
  message: string;
}

/**
 * What {@link WebViewer.close} closes:
 * the recordings loaded from these URLs, which also stops loading from them,
 * these recordings by store id, the active recording (`"current"`), or every recording (`"all"`).
 */
interface StoreIdsTarget {
  storeIds: StoreId[];
}

interface LegacyStoreIdsTarget {
  /** @deprecated Since 0.43.0. Use `storeIds` instead. */
  "store_ids": StoreId[];
}

export type CloseTarget =
  | (string & {})
  | string[]
  | StoreIdsTarget
  | LegacyStoreIdsTarget
  | "current"
  | "all";

/** How {@link WebViewer.open} opens URLs. */
export interface OpenOptions {
  /** Whether to also show what it opens, making a recording the active one. Defaults to `true`. */
  navigate?: boolean;
}

/** What {@link WebViewer.open} did. */
export interface OpenResult {
  /** The store ids of the recordings loaded from the URLs. */
  storeIds: RecordingStoreId[];
  /** @deprecated Since 0.43.0. Use `storeIds` instead. */
  "store_ids": RecordingStoreId[];

  /**
   * The URLs, as passed in, whose sources had not finished their initial load when the viewer answered.
   *
   * Empty once everything is loaded. Otherwise, poll {@link WebViewer.getState} until its `loading` list is empty.
   */
  loading: string[];
}

/** Which part of a recording or dataset a schema describes. */
export interface SchemaOptions {
  /** Only describe this entity and the entities below it, e.g. `/world/robot`. */
  entityPath?: string;
  /** @deprecated Since 0.43.0. Use `entityPath` instead. */
  "entity_path"?: string;

  /** How many entities to describe at most. Defaults to 100, or to 10000 with `pathsOnly`. */
  maxEntities?: number;
  /** @deprecated Since 0.43.0. Use `maxEntities` instead. */
  "max_entities"?: number;

  /** Name the entities without describing what is logged on them. */
  pathsOnly?: boolean;
  /** @deprecated Since 0.43.0. Use `pathsOnly` instead. */
  "paths_only"?: boolean;
}

/** Which recording {@link WebViewer.getRecordingSchema} describes, and which part of it. */
export interface RecordingSchemaOptions extends SchemaOptions {
  /** The recording, as a store id from {@link WebViewer.getState}. If omitted, the active recording. */
  recording?: RecordingStoreId;
}

/** A recording's timelines, entities, and the components logged on them. */
export interface RecordingSchema {
  /** The recording that was described, as a store id. */
  recording: RecordingStoreId;

  /** The recording's timelines with their time ranges. */
  timelines: ViewerTimeline[];

  /** The matching entities in path order. */
  entities: EntitySchema[];

  /** How many matching entities got left out because of `maxEntities`. */
  omittedEntities: number;
  /** @deprecated Since 0.43.0. Use `omittedEntities` instead. */
  "omitted_entities": number;
}

/** An entity, with every component ever logged on it. */
export interface EntitySchema {
  /** The entity path, e.g. `/world/robot/joints`. */
  entityPath: string;
  /** @deprecated Since 0.43.0. Use `entityPath` instead. */
  "entity_path": string;

  /** The components logged on this entity. Empty with `pathsOnly`. */
  components: ComponentSchema[];
}

/** A component logged on an entity. */
export interface ComponentSchema {
  /** The name to pass along in a query, e.g. `Points3D:positions`. */
  component: string;

  /** The archetype it was logged through, e.g. `rerun.archetypes.Points3D`, if any. */
  archetype?: string;

  /** The semantic type of a value, e.g. `rerun.components.Position3D`, if any. */
  componentType?: string;
  /** @deprecated Since 0.43.0. Use `componentType` instead. */
  "component_type"?: string;

  /** Arrow datatype of a single value, e.g. `FixedSizeList(3 x non-null Float32)`. */
  datatype: string;

  /**
   * Whether the component has ever been logged statically.
   *
   * It may also have temporal data, which takes precedence where it exists.
   */
  hasStatic: boolean;
  /** @deprecated Since 0.43.0. Use `hasStatic` instead. */
  "has_static": boolean;
}

/** Which dataset {@link WebViewer.getDatasetSchema} and {@link WebViewer.getDatasetAssetsSchema} describe. */
export interface DatasetSchemaOptions extends SchemaOptions {
  /** A dataset `url` from the `servers` of {@link WebViewer.getState}. If omitted, the dataset on screen. */
  datasetUrl?: string;
  /** @deprecated Since 0.43.0. Use `datasetUrl` instead. */
  "dataset_url"?: string;
}

/** The entities of a dataset's segments, and the components logged on them. */
export interface DatasetSchema {
  /** The names of the dataset's timelines. */
  timelines: string[];

  /** The matching entities in path order. */
  entities: EntitySchema[];

  /** How many matching entities got left out because of `maxEntities`. */
  omittedEntities: number;
  /** @deprecated Since 0.43.0. Use `omittedEntities` instead. */
  "omitted_entities": number;

  /** The ids of the dataset's assets, which {@link WebViewer.getDatasetAssetsSchema} describes. */
  assetIds: string[];
  /** @deprecated Since 0.43.0. Use `assetIds` instead. */
  "asset_ids": string[];
}

/** An asset of a dataset: static data shared by its segments, such as a robot's meshes. */
export interface DatasetAssetSchema {
  /** The asset's id, which is its segment id in the dataset's asset dataset. */
  assetId: string;
  /** @deprecated Since 0.43.0. Use `assetId` instead. */
  "asset_id": string;

  /** The matching entities in path order. */
  entities: EntitySchema[];

  /** How many matching entities of this asset got left out because of `maxEntities`. */
  omittedEntities: number;
  /** @deprecated Since 0.43.0. Use `omittedEntities` instead. */
  "omitted_entities": number;
}

/** A column of a table, as {@link WebViewer.getTableSchema} reports it. */
export interface TableColumnSchema {
  /** The Arrow field name, which table blueprints use, e.g. `rerun_segment_id`. */
  name: string;

  /** The name in the table's header. */
  displayName: string;
  /** @deprecated Since 0.43.0. Use `displayName` instead. */
  "display_name": string;

  /** Arrow datatype of the column, e.g. `Utf8`. */
  datatype: string;
}

/** What {@link WebViewer.takeScreenshot} captures. */
export interface ScreenshotOptions {
  /** The view to capture, as a `viewId` from {@link WebViewer.getState}. If omitted, the whole viewer. */
  viewId?: Uuid;
  /** @deprecated Since 0.43.0. Use `viewId` instead. */
  "view_id"?: Uuid;

  /** Longest side of the image, in pixels. A larger screenshot is scaled down to fit. If omitted, full resolution. */
  maxSize?: number;
  /** @deprecated Since 0.43.0. Use `maxSize` instead. */
  "max_size"?: number;
}

/** A rectangle in the viewer, in logical points from its top-left corner. */
export interface ScreenRect {
  /** Distance from the left edge of the viewer to the left edge of the rectangle. */
  x: number;

  /** Distance from the top edge of the viewer to the top edge of the rectangle. */
  y: number;

  /** Must be positive. */
  w: number;

  /** Must be positive. */
  h: number;
}

/** A registered view type and its discovery metadata. */
export interface ViewerViewType {
  /** Exact identifier to use in a blueprint. */
  classIdentifier: string;
  displayName: string;
  description: string;
  keywords: string[];
}

/** The recording or server that a command acts on. */
export interface CommandTarget {
  /** The recording for a `recording` command, as a store id. If omitted, the active recording. */
  recording?: RecordingStoreId;

  /** The server for a `server` command, e.g. `rerun+http://127.0.0.1:51234`. If omitted, the selected server. */
  serverOrigin?: string;
  /** @deprecated Since 0.43.0. Use `serverOrigin` instead. */
  "server_origin"?: string;
}

/** Which commands {@link WebViewer.listCommands} lists. */
export interface ListCommandsOptions extends CommandTarget {
  /** Only these commands, by id, in this order. If omitted, every command, sorted by id. */
  ids?: string[];

  /** Include commands that cannot run right now. Commands named in `ids` are always included. */
  includeUnavailable?: boolean;
  /** @deprecated Since 0.43.0. Use `includeUnavailable` instead. */
  "include_unavailable"?: boolean;
}

/** How {@link WebViewer.runCommand} runs a command. */
export interface RunCommandOptions extends CommandTarget {
  /** Allow a command that opens a native modal, which blocks the viewer until a person closes it. */
  allowBlockingNativeModal?: boolean;
  /** @deprecated Since 0.43.0. Use `allowBlockingNativeModal` instead. */
  "allow_blocking_native_modal"?: boolean;
}

/** What kind of thing a {@link ViewerCommand} acts on. */
export type CommandScope = "global" | "recording" | "server" | "table";

/** A command palette command, as {@link WebViewer.listCommands} describes it. */
export interface ViewerCommand {
  /** The id for {@link WebViewer.runCommand}. */
  id: string;

  /** The name shown in the palette. */
  title: string;

  /** What the command does. */
  description: string;

  /** What kind of thing it acts on. */
  scope: CommandScope;

  /** Keyboard shortcuts as the viewer shows them, e.g. `⌘K`. Primary first. */
  shortcuts: string[];

  /** Whether {@link WebViewer.runCommand} would run it now. */
  available: boolean;

  /** Whether it opens a native modal, which blocks the viewer until a person closes it. */
  blockingNativeModal: boolean;
  /** @deprecated Since 0.43.0. Use `blockingNativeModal` instead. */
  "blocking_native_modal": boolean;

  /** Whether it discards data, blueprint edits, a server, or the viewer itself. */
  destructive: boolean;

  /** The store id, server origin, or table within `scope` that it would act on now, if any. */
  target?: string;
}

/**
 * Whose blueprint {@link WebViewer.getBlueprint} reads, or {@link WebViewer.setBlueprint} writes.
 *
 * A recording's blueprint has its layout, views, their properties, and overrides.
 * Every recording of the same application shares one.
 * A table's blueprint has its row filters, layout, and column settings.
 */
export type BlueprintTarget =
  | {
      /** A recording's blueprint. */
      kind: "recording";

      /** The recording, as a store id. If omitted, the active recording. */
      recording?: RecordingStoreId;
    }
  | {
      /** A table's blueprint. */
      kind: "table";

      /**
       * A dataset or table `url` from the `servers` of {@link WebViewer.getState},
       * or a server's `origin` for its list of entries. If omitted, the table on screen.
       */
      tableUrl?: string;
      /** @deprecated Since 0.43.0. Use `tableUrl` instead. */
      "table_url"?: string;
    };

/**
 * A whole blueprint, keyed by blueprint entity path, then archetype, then field.
 * See [Blueprints](https://rerun.io/docs/viewer/blueprints-and-views/blueprints).
 *
 * An integer is a `number` if a `number` holds it exactly, and a `bigint` if not, such as most nanosecond times.
 * So the same field can be either, depending on its value: handle both, e.g. with `BigInt(value)`.
 * `JSON.stringify` throws on a `bigint`, but {@link WebViewer.setBlueprint} takes them.
 *
 * ```json
 * { "/table": { "TableBlueprint": { "layout": "Cards" } } }
 * ```
 */
export type BlueprintJson = Record<string, Record<string, Record<string, unknown>>>;

/** The blueprint that {@link WebViewer.setBlueprint} wrote. */
export interface BlueprintRef {
  /** The blueprint store. */
  blueprintId: BlueprintStoreId;
  /** @deprecated Since 0.43.0. Use `blueprintId` instead. */
  "blueprint_id": BlueprintStoreId;

  /** The recording whose blueprint it is, as a store id, if it is a recording's. */
  recording?: RecordingStoreId;

  /** The table whose blueprint it is, if it is a table's that has a URL. */
  tableUrl?: string;
  /** @deprecated Since 0.43.0. Use `tableUrl` instead. */
  "table_url"?: string;
}

/** A blueprint that {@link WebViewer.getBlueprint} read. */
export interface Blueprint extends BlueprintRef {
  /** The whole blueprint. */
  blueprint: BlueprintJson;
}

/** Which transform frames and edges {@link WebViewer.getTransformGraph} includes. */
export interface TransformGraphOptions {
  /** The recording, as a store id. If omitted, the active recording. */
  recording?: RecordingStoreId;

  /** Frames derived from entity paths, explicitly named frames, or both. Defaults to `all`. */
  frameFilter?: "entity_path" | "named" | "all";
  /** @deprecated Since 0.43.0. Use `frameFilter` instead. */
  "frame_filter"?: "entity_path" | "named" | "all";

  /** Static edges, temporal edges, or both. Defaults to `all`. */
  edgeFilter?: "static" | "temporal" | "all";
  /** @deprecated Since 0.43.0. Use `edgeFilter` instead. */
  "edge_filter"?: "static" | "temporal" | "all";

  /** Include affine matrices and pinhole intrinsics, resolutions, and camera axes. */
  includeTransforms?: boolean;
  /** @deprecated Since 0.43.0. Use `includeTransforms` instead. */
  "include_transforms"?: boolean;
}

/** A recording's transform graph at its time cursor. */
export interface TransformGraph {
  /** The recording that was queried, as a store id. */
  recording: RecordingStoreId;

  /**
   * The query, options, missing chunks, frames, and edges, as the viewer reports them.
   *
   * Integers are a `number` or a `bigint`, as in {@link BlueprintJson}.
   */
  graph: Record<string, unknown>;
}

/** The public {@link ViewerTimeline} of a timeline in a viewer control response. */
function timelineOf(timeline: ProtoViewerTimeline): ViewerTimeline {
  return withLegacyFields({
    name: timeline.timeline?.name ?? "",
    timeType: timeTypeOf(timeline.timeType),
    timeRange: timeline.timeRange && {
      start: timeline.timeRange.start,
      end: timeline.timeRange.end,
    },
    currentTime: timeline.currentTime?.time,
  });
}

/** The public {@link EntitySchema} of an entity in a schema response. */
function entitySchemaOf(entity: RecordingEntitySchema): EntitySchema {
  return withLegacyFields({
    entityPath: entity.entityPath,
    components: entity.components.map((component) => ({
      component: component.component,
      archetype: component.archetype,
      componentType: component.componentType,
      datatype: component.datatype,
      hasStatic: component.hasStatic ?? false,
    })),
  });
}

/** The request fields for `options`, shared by every schema operation. */
function schemaRequestOf({ entityPath, maxEntities, pathsOnly }: SchemaOptions) {
  return { entityPath, maxEntities, pathsOnly };
}

/** The public {@link ViewerCommand} of a command in a viewer control response. */
function viewerCommandOf(command: ProtoViewerCommand): ViewerCommand {
  return withLegacyFields({
    id: command.id,
    title: command.title,
    description: command.description,
    scope: command.scope as CommandScope,
    shortcuts: command.shortcuts,
    available: command.available,
    blockingNativeModal: command.blockingNativeModal,
    destructive: command.destructive,
    target: command.target,
  });
}

/** The `target` of a close request. */
function closeTargetOf(target: CloseTarget) {
  if (target === "current") {
    return { case: "current" as const, value: true };
  }
  if (target === "all") {
    return { case: "all" as const, value: true };
  }
  if (typeof target === "string" || Array.isArray(target)) {
    return { case: "urls" as const, value: { urls: Array.isArray(target) ? target : [target] } };
  }
  // Plain JavaScript callers are not type-checked, and guessing here could close everything.
  if (typeof target === "object" && target !== null && "storeIds" in target) {
    return { case: "storeIds" as const, value: { storeIds: target.storeIds } };
  }
  throw new Error('close expects URLs, `{ storeIds }`, `"current"`, or `"all"`');
}

/** The `target` of a blueprint request. */
function blueprintTargetOf(target: BlueprintTarget) {
  return target.kind === "recording"
    ? { case: "recording" as const, value: { storeId: target.recording } }
    : { case: "table" as const, value: { tableUrl: target.tableUrl } };
}

const FRAME_FILTERS = {
  "entity_path": GetTransformGraphRequestFrameFilter.ENTITY_PATH,
  named: GetTransformGraphRequestFrameFilter.NAMED,
  all: GetTransformGraphRequestFrameFilter.ALL,
} as const;

const EDGE_FILTERS = {
  static: GetTransformGraphRequestEdgeFilter.STATIC,
  temporal: GetTransformGraphRequestEdgeFilter.TEMPORAL,
  all: GetTransformGraphRequestEdgeFilter.ALL,
} as const;

/**
 * Where to move the time cursor to, for {@link WebViewer.setTime}, and on which recording and timeline.
 *
 * Set at most one of `sequence`, `durationNanos`, and `timestampNanos`.
 * Without any of them, the cursor stays where it is on the timeline,
 * so you can switch timelines, or play or pause, on their own.
 */
export type SetTimeOptions = SetTimeTarget &
  (
    | TimeValue
    | {
        sequence?: never;
        durationNanos?: never;
        timestampNanos?: never;
        "duration_nanos"?: never;
        "timestamp_nanos"?: never;
      }
  );

/** The recording and timeline that {@link WebViewer.setTime} acts on. */
export interface SetTimeTarget {
  /** The timeline to seek on. If omitted, the viewer uses its active timeline. */
  timeline?: string;

  /**
   * Whether to pause, play, or follow the latest data. If omitted, playback continues as it was.
   *
   * Following moves the cursor to the latest data, so it cannot be combined with a time.
   */
  playState?: PlayState;
  /** @deprecated Since 0.43.0. Use `playState` instead. */
  "play_state"?: PlayState;

  /** The recording to seek, as a store id from {@link WebViewer.getState}. If omitted, the active recording. */
  recording?: RecordingStoreId;
}

/**
 * A time on a timeline, as exactly one of a sequence index, a duration, or a timestamp.
 *
 * Integer values only: a fractional `number` is rejected.
 */
export type TimeValue =
  | {
      /** A sequence index. */
      sequence: number | bigint;
      durationNanos?: never;
      timestampNanos?: never;
      /** @deprecated Since 0.43.0. Use `durationNanos` instead. */
      "duration_nanos"?: never;
      /** @deprecated Since 0.43.0. Use `timestampNanos` instead. */
      "timestamp_nanos"?: never;
    }
  | {
      sequence?: never;
      /** A duration, in nanoseconds. */
      durationNanos: number | bigint;
      timestampNanos?: never;
      /** @deprecated Since 0.43.0. Use `durationNanos` instead. */
      "duration_nanos"?: number | bigint;
      /** @deprecated Since 0.43.0. Use `timestampNanos` instead. */
      "timestamp_nanos"?: never;
    }
  | {
      sequence?: never;
      durationNanos?: never;
      /**
       * A timestamp, in nanoseconds since the Unix epoch.
       *
       * A `number` is only exact up to 2^53, which today's timestamps exceed, so prefer a `bigint`.
       */
      timestampNanos: number | bigint;
      /** @deprecated Since 0.43.0. Use `durationNanos` instead. */
      "duration_nanos"?: never;
      /** @deprecated Since 0.43.0. Use `timestampNanos` instead. */
      "timestamp_nanos"?: number | bigint;
    }
  | {
      sequence?: never;
      durationNanos?: number | bigint;
      timestampNanos?: never;
      /** @deprecated Since 0.43.0. Use `durationNanos` instead. */
      "duration_nanos": number | bigint;
      /** @deprecated Since 0.43.0. Use `timestampNanos` instead. */
      "timestamp_nanos"?: never;
    }
  | {
      sequence?: never;
      durationNanos?: never;
      timestampNanos?: number | bigint;
      /** @deprecated Since 0.43.0. Use `durationNanos` instead. */
      "duration_nanos"?: never;
      /** @deprecated Since 0.43.0. Use `timestampNanos` instead. */
      "timestamp_nanos": number | bigint;
    };

/** Why an operation of the viewer's control API failed, as a gRPC status code. */
export type ViewerControlErrorCode =
  | "INVALID_ARGUMENT"
  | "NOT_FOUND"
  | "FAILED_PRECONDITION"
  | "INTERNAL";

/** A failed operation of the viewer's control API, such as {@link WebViewer.setTime}. */
export class ViewerControlError extends Error {
  override name = "ViewerControlError";

  /** Why the operation failed. */
  readonly code: ViewerControlErrorCode;

  constructor(code: ViewerControlErrorCode, message: string) {
    super(message);
    this.code = code;
  }
}

/** A {@link ViewerControlError} for an error that the wasm viewer rejected with. */
function asViewerControlError(err: unknown): unknown {
  if (err instanceof Error && err.name === "ViewerControlError" && "code" in err) {
    return new ViewerControlError(err.code as ViewerControlErrorCode, err.message);
  }
  return err;
}

/** An operation of the viewer's control protocol, e.g. `"getState"`. */
type Operation = NonNullable<ViewerControlRequest["kind"]["case"]>;

/** The request of the operation `K`. */
type OperationRequest<K extends Operation> = Extract<
  NonNullable<MessageInitShape<typeof ViewerControlRequestSchema>["kind"]>,
  { case: K }
>["value"];

/** The response of the operation `K`. */
type OperationResponse<K extends Operation> = Extract<
  ViewerControlResponse["kind"],
  { case: K }
>["value"];

/** The time that `options` seeks to, in the timeline's own unit, if any. */
function timeOf(options: SetTimeOptions): bigint | undefined {
  // The type allows only one, but plain JavaScript callers are not type-checked.
  const values = [options.sequence, options.durationNanos, options.timestampNanos].filter(
    (value) => value !== undefined,
  );
  if (1 < values.length) {
    throw new Error(
      "setTime expects at most one of `sequence`, `durationNanos`, and `timestampNanos`",
    );
  }
  const [value] = values;
  if (value === undefined) {
    return undefined;
  }
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(`setTime expects an integer time, got ${value}`);
  }
  return BigInt(value);
}

let getWasmBindgen: (() => typeof wasmBindgen) | null = null;
let wasmModule: WebAssembly.Module | null = null;

/**
 * Feature-detect WebAssembly SIMD (`simd128`).
 *
 * The viewer .wasm is compiled with `-Ctarget-feature=+simd128`, so a browser
 * without SIMD support will fail to instantiate the module with a cryptic
 * `CompileError`. We probe up-front and surface a clear error instead.
 *
 * The probe is a minimal module that uses the `v128.any_true` instruction.
 * Supported in: Chrome 91+, Firefox 89+, Safari 16.4+.
 */
function hasWasmSimd(): boolean {
  try {
    return WebAssembly.validate(
      new Uint8Array([
        0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0,
        253, 15, 253, 98, 11,
      ]),
    );
  } catch {
    return false;
  }
}

const UNSUPPORTED_BROWSER_MESSAGE =
  "Your browser is too old to run the Rerun Viewer. " +
  "The Viewer requires WebAssembly SIMD support, available in " +
  "Chrome 91+, Firefox 89+, Safari 16.4+, or any modern Chromium-based browser. " +
  "Please update your browser and try again.";

// The public export API uses streams so the private Wasm API can switch to incremental encoding
// without requiring a breaking JavaScript API change.
function exportStream(bytes: Uint8Array): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(bytes);
      controller.close();
    },
  });
}

async function fetchViewerJs(): Promise<() => typeof wasmBindgen> {
  // @ts-ignore
  return (await import("./re_viewer.js")).default;
}

async function fetchViewerWasm(
  baseUrl?: string,
  onProgress?: (received: number, total: number | null) => void,
): Promise<Response> {
  //!<INLINE-MARKER-OPEN>
  const url = baseUrl
    ? new URL("./re_viewer_bg.wasm", baseUrl)
    : new URL("./re_viewer_bg.wasm", import.meta.url);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch viewer Wasm: ${response.status} ${response.statusText}`);
  }
  return wrapFetchWithProgress(response, onProgress);
  //!<INLINE-MARKER-CLOSE>
}

/**
 * Estimates total uncompressed bytes for progress display.
 * This is a rough estimate — do NOT use for truncation detection.
 */
function estimateTotalBytes(response: Response): number | null {
  // When served with `rerun-final-length`, use that (set by `re_web_viewer_server`).
  const finalLength = response.headers.get("rerun-final-length");
  if (finalLength != null) return parseInt(finalLength, 10);

  // GCS custom metadata, needed since our wasm there is Brotli-compressed.
  const uncompressed = response.headers.get("x-goog-meta-uncompressed-size");
  if (uncompressed != null) return parseInt(uncompressed, 10);

  if (response.headers.get("content-encoding") === "gzip") {
    // Fall back to content-length * 3 (good empirical approximation for gzip'd wasm).
    const cl = response.headers.get("content-length");
    if (cl != null) return parseInt(cl, 10) * 3;
  }

  // Uncompressed: content-length is the exact size.
  const cl = response.headers.get("content-length");
  if (cl != null) return parseInt(cl, 10);

  return null;
}

/**
 * Wraps a fetch response to track download progress.
 */
function wrapFetchWithProgress(
  response: Response,
  onProgress?: (received: number, total: number | null) => void,
): Response {
  const totalBytes = estimateTotalBytes(response);

  if (!response.body) return response;

  let received = 0;
  const body = response.body;
  const tracked = new ReadableStream({
    async start(controller) {
      const reader = body.getReader();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        received += value.byteLength;
        onProgress?.(received, totalBytes);
        controller.enqueue(value);
      }
      controller.close();
    },
  });

  return new Response(tracked, {
    status: response.status,
    statusText: response.statusText,
    headers: response.headers,
  });
}

function formatMib(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1) + " MiB";
}

async function load(
  baseUrl?: string,
  onProgress?: (received: number, total: number | null) => void,
): Promise<typeof wasmBindgen.WebHandle> {
  if (!hasWasmSimd()) {
    throw new Error(UNSUPPORTED_BROWSER_MESSAGE);
  }

  // instantiate wbg globals+module for every invocation of `load`,
  // but don't load the JS/Wasm source every time
  if (!getWasmBindgen || !wasmModule) {
    [getWasmBindgen, wasmModule] = await Promise.all([
      fetchViewerJs(),
      WebAssembly.compileStreaming(fetchViewerWasm(baseUrl, onProgress)),
    ]);
  }
  let bindgen = getWasmBindgen();
  await bindgen({ "module_or_path": wasmModule });
  return class extends bindgen.WebHandle {
    free() {
      super.free();
      // @ts-ignore
      bindgen.deinit();
    }
  };
}

let minimizeCurrentFullscreenViewer: (() => void) | null = null;

export type Panel = "top" | "blueprint" | "selection" | "time";
export type PanelState = "hidden" | "collapsed" | "expanded";
export type Backend = "webgpu" | "webgl";
export type VideoDecoder = "auto" | "prefer_software" | "prefer_hardware";

interface LoginRedirects {
  /** URL to redirect to after successful OAuth login (e.g. "/signed-in" or "https://example.com/signed-in"). */
  signedInUrl?: string;
  /** @deprecated Since 0.43.0. Use `signedInUrl` instead. */
  "signed_in_url"?: string;
  /** URL to redirect to after logout (e.g. "/signed-out" or "https://example.com/signed-out"). */
  signedOutUrl?: string;
  /** @deprecated Since 0.43.0. Use `signedOutUrl` instead. */
  "signed_out_url"?: string;
}

/** OAuth redirects, accepting either spelling of each required URL. */
export type LoginOptions = LoginRedirects &
  (
    | { signedInUrl: string }
    | { /** @deprecated Since 0.43.0. Use `signedInUrl` instead. */ "signed_in_url": string }
  ) &
  (
    | { signedOutUrl: string }
    | { /** @deprecated Since 0.43.0. Use `signedOutUrl` instead. */ "signed_out_url": string }
  );

// NOTE: When changing these options, consider how it affects the `web-viewer-react` package:
//       - Should this option be exposed?
//       - Should changing this option result in the viewer being restarted?
export interface WebViewerOptions {
  /** Url to the example manifest. Unused if `hideWelcomeScreen` is set to `true`. */
  manifestUrl?: string;
  /** @deprecated Since 0.43.0. Use `manifestUrl` instead. */
  "manifest_url"?: string;

  /** The render backend used by the viewer. Either "webgl" or "webgpu". Prefers "webgpu". */
  renderBackend?: Backend;
  /** @deprecated Since 0.43.0. Use `renderBackend` instead. */
  "render_backend"?: Backend;

  /** Video decoder config used by the viewer. Either "auto", "prefer_software" or "prefer_hardware". */
  videoDecoder?: VideoDecoder;
  /** @deprecated Since 0.43.0. Use `videoDecoder` instead. */
  "video_decoder"?: VideoDecoder;

  /**
   * An upper limit on how much memory the viewer uses, e.g. "500MB" or "50%".
   *
   * When this limit is reached, the viewer drops the oldest data.
   * A percentage is relative to the 4 GiB a Wasm process can use.
   * The limit is capped at 2.5 GB, which is also the default.
   * If not set, uses the limit from the viewer settings.
   */
  memoryLimit?: string;
  /** @deprecated Since 0.43.0. Use `memoryLimit` instead. */
  "memory_limit"?: string;

  /** If set to `true`, hides the welcome screen, which contains our examples. Defaults to `false`. */
  hideWelcomeScreen?: boolean;
  /** @deprecated Since 0.43.0. Use `hideWelcomeScreen` instead. */
  "hide_welcome_screen"?: boolean;

  /**
   * Override whether the viewer checks for newer Rerun releases on startup.
   * Set to `false` to disable these checks.
   * If not set, uses the saved user preference (enabled by default).
   */
  checkForUpdatesOnStartup?: boolean;
  /** @deprecated Since 0.43.0. Use `checkForUpdatesOnStartup` instead. */
  "check_for_updates_on_startup"?: boolean;

  /**
   * Allow the viewer to handle fullscreen mode.
   * This option sets canvas style so is not recommended if you are doing anything custom,
   * or are embedding the viewer in an iframe.
   *
   * Defaults to `false`.
   */
  allowFullscreen?: boolean;
  /** @deprecated Since 0.43.0. Use `allowFullscreen` instead. */
  "allow_fullscreen"?: boolean;

  /**
   * Enable the history feature of the viewer.
   *
   * This is only relevant when `hideWelcomeScreen` is `false`,
   * as it's currently only used to allow going between the welcome screen and examples.
   *
   * Defaults to `false`.
   */
  enableHistory?: boolean;
  /** @deprecated Since 0.43.0. Use `enableHistory` instead. */
  "enable_history"?: boolean;

  /** The CSS width of the canvas. */
  width?: string;

  /** The CSS height of the canvas. */
  height?: string;

  /** The fallback token to use, if any.
   *
   * The fallback token behaves similarly to the `REDAP_TOKEN` env variable. If set in the
   * enclosing notebook environment, it should be used to set the fallback token.
   */
  fallbackToken?: string;
  /** @deprecated Since 0.43.0. Use `fallbackToken` instead. */
  "fallback_token"?: string;

  /**
   * The color theme to use.
   *
   * If not set, the viewer uses the previously persisted theme preference or defaults to "system".
   */
  theme?: "dark" | "light" | "system";

  /**
   * Enable OAuth login in the viewer.
   *
   * When set, the viewer shows login UI and uses the provided URLs for OAuth redirects.
   *
   * To use this:
   * 1. Host the `signed-in.html` and `signed-out.html` pages alongside your viewer.
   *    Templates can be found at:
   *    - https://github.com/rerun-io/rerun/blob/main/crates/top/re_web_viewer_server/web_viewer/signed-in.html
   *    - https://github.com/rerun-io/rerun/blob/main/crates/top/re_web_viewer_server/web_viewer/signed-out.html
   * 2. Set the URLs to those pages here.
   * 3. Contact your Rerun representative to have the redirect URLs
   *    and origin whitelisted in the OAuth configuration.
   *
   * When not set (default), login UI is hidden. Token-based auth still works.
   */
  login?: LoginOptions;
}

// `AppOptions` and `WebViewerOptions` must be compatible
// otherwise we need to restructure how we pass options to the viewer

/**
 * The public interface is @see {WebViewerOptions}. This adds a few additional, internal options.
 *
 * @private
 */
export interface AppOptions extends WebViewerOptions {
  /** The url that's used when sharing web viewer urls
   *
   * If not set, the viewer will use the url of the page it is embedded in.
   */
  viewerBaseUrl?: string;
  /** @deprecated Since 0.43.0. Use `viewerBaseUrl` instead. */
  "viewer_base_url"?: string;

  /** Whether the viewer is running in a notebook. */
  notebook?: boolean;

  url?: string;
  panelStateOverrides?: Partial<{
    [K in Panel]: PanelState;
  }>;
  /** @deprecated Since 0.43.0. Use `panelStateOverrides` instead. */
  "panel_state_overrides"?: Partial<{
    [K in Panel]: PanelState;
  }>;
  onViewerEvent?: (eventJson: string) => void;
  /** @deprecated Since 0.43.0. Use `onViewerEvent` instead. */
  "on_viewer_event"?: (eventJson: string) => void;
  fullscreen?: FullscreenOptions;
}

// Types are based on `crates/top/re_viewer/src/event.rs`.
// Important: The event names defined here are `snake_case` versions
// of their `PascalCase` counterparts on the Rust side.
/** An event produced in the Viewer. */
export type ViewerEvent =
  | PlayEvent
  | PauseEvent
  | TimeUpdateEvent
  | TimelineChangeEvent
  | SelectionChangeEvent
  | RecordingOpenEvent;

/**
 * Properties available on all {@link ViewerEvent} types.
 */
export interface ViewerEventBase {
  applicationId: string;
  /** @deprecated Since 0.43.0. Use `applicationId` instead. */
  "application_id": string;
  recordingId: string;
  /** @deprecated Since 0.43.0. Use `recordingId` instead. */
  "recording_id": string;
  /** The segment of the recording's dataset, if it was opened from a Rerun server. */
  segmentId?: string;
  /** @deprecated Since 0.43.0. Use `segmentId` instead. */
  "segment_id"?: string;
  /** @deprecated Since 0.43.0. Use `segmentId` instead. */
  "partition_id"?: string;
}

/**
 * Fired when the timeline starts playing.
 */
export interface PlayEvent extends ViewerEventBase {
  type: "play";
}

/**
 * Fired when the timeline stops playing.
 */
export interface PauseEvent extends ViewerEventBase {
  type: "pause";
}

/**
 * Fired when the timepoint changes.
 */
export interface TimeUpdateEvent extends ViewerEventBase {
  type: "time_update";
  time: number;
}

/**
 * Fired when a different timeline is selected.
 */
export interface TimelineChangeEvent extends ViewerEventBase {
  type: "timeline_change";
  timeline: string;
  time: number;
}

/**
 * Fired when the selection changes.
 *
 * This event is fired each time any part of the event payload changes,
 * this includes for example clicking on different parts of the same
 * entity in a 2D or 3D view.
 */
export interface SelectionChangeEvent extends ViewerEventBase {
  type: "selection_change";
  items: SelectionChangeItem[];
}

/**
 * Fired when a new recording is opened in the Viewer.
 *
 * For `rrd` file or stream, a recording is considered "open" after
 * enough information about the recording, such as its ID and source,
 * is received.
 *
 * Contains some basic information about the origin of the recording.
 */
export interface RecordingOpenEvent extends ViewerEventBase {
  type: "recording_open";

  /**
   * Where the recording came from.
   *
   * The value should be considered unstable, which is why we don't
   * list the possible values here.
   */
  source: string;

  /**
   * Version of the SDK used to create this recording.
   *
   * Uses semver format.
   */
  version?: string;
}

// A bit of TypeScript metaprogramming to automatically produce a
// mapping of event names to event payloads given the above type
// definitions.

// Yield the event with type `K`.
type GetViewerEvent<K> = Extract<ViewerEvent, { type: K }>;

// `ViewerEvent` is a union of all events, so its `type` field
// is a union of all `type` fields.
type ViewerEventNames = ViewerEvent["type"];

// For every event, get its payload type.
type ViewerEventMap = {
  [K in ViewerEventNames]: GetViewerEvent<K>;
};

/**
 * Selected an entity, or an instance of an entity.
 *
 * If the entity was selected within a view, then this also
 * includes the view's name.
 *
 * If the entity was selected within a 2D or 3D space view,
 * then this also includes the position.
 */
export interface EntityItem {
  type: "entity";

  entityPath: string;
  /** @deprecated Since 0.43.0. Use `entityPath` instead. */
  "entity_path": string;
  instanceId?: number;
  /** @deprecated Since 0.43.0. Use `instanceId` instead. */
  "instance_id"?: number;
  viewName?: string;
  /** @deprecated Since 0.43.0. Use `viewName` instead. */
  "view_name"?: string;
  position?: [number, number, number];
}

/** Selected a view. */
export interface ViewItem {
  type: "view";
  viewId: string;
  /** @deprecated Since 0.43.0. Use `viewId` instead. */
  "view_id": string;
  viewName: string;
  /** @deprecated Since 0.43.0. Use `viewName` instead. */
  "view_name": string;
}

/** Selected a container. */
export interface ContainerItem {
  type: "container";
  containerId: string;
  /** @deprecated Since 0.43.0. Use `containerId` instead. */
  "container_id": string;
  containerName: string;
  /** @deprecated Since 0.43.0. Use `containerName` instead. */
  "container_name": string;
}

/** A single item in a selection. */
export type SelectionChangeItem = EntityItem | ViewItem | ContainerItem;

interface FullscreenCallbacks {
  getState?: () => boolean;
  /** @deprecated Since 0.43.0. Use `getState` instead. */
  "get_state"?: () => boolean;
  onToggle?: () => void;
  /** @deprecated Since 0.43.0. Use `onToggle` instead. */
  "on_toggle"?: () => void;
}

type FullscreenOptions = FullscreenCallbacks &
  (
    | { getState: () => boolean }
    | { /** @deprecated Since 0.43.0. Use `getState` instead. */ "get_state": () => boolean }
  ) &
  (
    | { onToggle: () => void }
    | { /** @deprecated Since 0.43.0. Use `onToggle` instead. */ "on_toggle": () => void }
  );

export interface WebViewerEvents extends ViewerEventMap {
  fullscreen: boolean;
  ready: void;
}

// This abomination is a mapped type with key filtering, and is used to split the events
// into those which take no value in their callback, and those which do.
// https://www.typescriptlang.org/docs/handbook/2/mapped-types.html#key-remapping-via-as
export type EventsWithValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void
    ? never
    : K]: WebViewerEvents[K] extends any[] ? WebViewerEvents[K] : [WebViewerEvents[K]];
};

export type EventsWithoutValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void ? K : never]: WebViewerEvents[K];
};

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** `value` with every object key in camelCase. Viewer events have no user-defined keys. */
function camelCaseKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(camelCaseKeys);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, child]) => [
      key.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase()),
      camelCaseKeys(child),
    ]),
  );
}

/** Resolve a potentially relative URL against the current origin. */
function resolveAbsoluteUrl(url: string): string {
  return new URL(url, window.location.href).toString();
}

/**
 * Rerun Web Viewer
 *
 * ```ts
 * const viewer = new WebViewer();
 * await viewer.start();
 * ```
 *
 * Data may be provided to the Viewer as:
 * - An HTTP file URL, e.g. `viewer.start("https://app.rerun.io/version/0.38.1/examples/dna.rrd")`
 * - A Rerun gRPC URL, e.g. `viewer.start("rerun+http://127.0.0.1:9876/proxy")`
 * - A stream of log messages, via {@link WebViewer.openChannel}.
 *
 * Callbacks may be attached for various events using {@link WebViewer.on}:
 *
 * ```ts
 * viewer.on("time_update", (time) => console.log(`current time: {time}`));
 * ```
 *
 * For the full list of available events, see {@link ViewerEvent}.
 *
 * Methods that read or drive the Viewer, like {@link WebViewer.getState} and {@link WebViewer.setTime},
 * return promises, because the Viewer handles each request on its next frame.
 * Some requests take even longer, such as a screenshot, or opening a URL, which waits for the data to load.
 * They are all async so that they work the same way.
 */
export class WebViewer {
  // NOTE: Using the handle requires wrapping all calls to its methods in try/catch.
  //       On failure, call `this.stop` to prevent a memory leak, then re-throw the error.
  #handle: WebHandle | null = null;
  #cachedViewerState: ViewerState | null = null;
  #canvas: HTMLCanvasElement | null = null;
  #loader: HTMLDivElement | null = null;
  #state: "ready" | "starting" | "stopped" = "stopped";
  #fullscreen = false;
  #allowFullscreen = false;

  constructor() {
    if (typeof window === "undefined" || typeof document === "undefined") {
      throw new Error(
        "WebViewer requires a browser document and canvas. " +
          "In Node.js, use Playwright to run the viewer in a browser, or launch the native headless viewer with `rerun --headless`.",
      );
    }
    injectStyle();
    setupGlobalEventListeners();
  }

  /**
   * Start the viewer.
   *
   * @param rrd URLs to `.rrd` files or gRPC connections to our SDK.
   * @param parent The element to attach the canvas onto.
   * @param options Web Viewer configuration.
   */
  async start(
    rrd: string | string[] | null,
    parent: HTMLElement | null,
    options: WebViewerOptions | null,
  ): Promise<void> {
    parent ??= document.body;
    options = normalizeOptions(options ?? {});

    this.#allowFullscreen = options.allowFullscreen || false;

    if (this.#state !== "stopped") return;
    this.#state = "starting";
    this.#clearLoader();

    this.#canvas = document.createElement("canvas");
    this.#canvas.style.width = options.width ?? "640px";
    this.#canvas.style.height = options.height ?? "360px";
    parent.append(this.#canvas);

    // Show loading progress bar
    this.#loader = document.createElement("div");
    this.#loader.innerHTML = `
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; background-color: #1c1c1c; font-family: sans-serif; color: white;">
        <div style="margin-bottom: 16px;">Loading Rerun\u2026</div>
        <div style="width: 200px;">
          <div style="background: #333; border-radius: 4px; height: 6px; overflow: hidden;">
            <div class="rerun-progress-bar" style="background: white; height: 100%; width: 0%; transition: width 0.2s;"></div>
          </div>
          <div class="rerun-progress-text" style="margin-top: 6px; font-size: 12px; color: #999;"></div>
        </div>
      </div>
    `;
    this.#loader.style.position = "absolute";
    this.#loader.style.inset = "0";
    parent.style.position = "relative";
    parent.append(this.#loader);

    const progressBar = this.#loader.querySelector(".rerun-progress-bar") as HTMLElement;
    const progressText = this.#loader.querySelector(".rerun-progress-text") as HTMLElement;

    const onProgress = (received: number, total: number | null) => {
      if (total != null && total > 0) {
        const pct = Math.min((received / total) * 100, 100);
        progressBar.style.width = pct.toFixed(1) + "%";
        progressText.textContent = `${Math.round(pct)}%`;
      } else {
        progressText.textContent = formatMib(received);
      }
    };

    // This yield appears to be necessary to ensure that the canvas is attached to the DOM
    // and visible. Without it we get occasionally get a panic about a failure to find a canvas
    // element with the given ID.
    await delay(0);

    let baseUrl: string | undefined = (options as any)?.baseUrl;
    if (baseUrl) {
      delete (options as any).baseUrl;
    }

    let WebHandleClass: typeof wasmBindgen.WebHandle;
    try {
      WebHandleClass = await load(baseUrl, onProgress);
    } catch (e) {
      this.#clearLoader();
      this.#fail("Failed to load rerun", String(e));
      throw e;
    }
    if (this.#state !== "starting") {
      this.#clearLoader();
      return;
    }

    const fullscreen = this.#allowFullscreen
      ? {
          getState: () => this.#fullscreen,
          onToggle: () => this.toggleFullscreen(),
        }
      : undefined;

    const onViewerEvent = (eventJson: string) => {
      // for notebooks/gradio, we can avoid a whole layer
      // of serde by sending over the raw json directly,
      // which will be deserialized in Python instead
      this.#dispatchRawEvent(eventJson);

      // for JS users, we dispatch the parsed event
      let event = withLegacyFields(camelCaseKeys(JSON.parse(eventJson))) as ViewerEvent;
      this.#dispatchEvent(event.type as any, event);
    };

    const login = options.login
      ? {
          signedInUrl: resolveAbsoluteUrl(options.login.signedInUrl!),
          signedOutUrl: resolveAbsoluteUrl(options.login.signedOutUrl!),
        }
      : undefined;

    this.#handle = new WebHandleClass({ ...options, login, fullscreen, onViewerEvent });
    try {
      await this.#handle.start(this.#canvas);
    } catch (e) {
      this.#clearLoader();
      this.#fail("Failed to start", String(e));
      throw e;
    }
    if (this.#state !== "starting") {
      this.#clearLoader();
      return;
    }

    this.#clearLoader();
    this.#state = "ready";
    this.#dispatchEvent("ready");

    if (rrd) {
      this.#addReceivers(rrd);
    }

    const checkForPanic = () => {
      if (this.#handle?.has_panicked()) {
        this.#fail("Rerun has crashed.", this.#handle?.panic_message());
      } else {
        let delayMs = 1000;
        setTimeout(checkForPanic, delayMs);
      }
    };

    checkForPanic();

    return;
  }

  #rawEvents: Set<(eventJson: string) => void> = new Set();
  #dispatchRawEvent(eventJson: string) {
    for (const callback of this.#rawEvents) {
      callback(eventJson);
    }
  }

  /** Internal interface */
  // NOTE: Callbacks passed to this function must NOT invoke any viewer methods!
  //       The `setTimeout` is omitted to avoid the 1-tick delay, as it is unnecessary,
  //       because this is only meant to be used for sending events to Jupyter/Gradio.
  //
  // Do not change this without searching for grepping for usage!
  private "_on_raw_event"(callback: (event: string) => void): () => void {
    this.#rawEvents.add(callback);
    return () => this.#rawEvents.delete(callback);
  }

  #eventMap: Map<keyof WebViewerEvents, Map<(...args: any[]) => void, { once: boolean }>> =
    new Map();

  #dispatchEvent<E extends keyof EventsWithValue>(event: E, ...args: EventsWithValue[E]): void;
  #dispatchEvent<E extends keyof EventsWithoutValue>(event: E): void;
  #dispatchEvent(event: any, ...args: any[]): void {
    // Dispatch events on next tick.
    // This is necessary because we may have been called somewhere deep within the viewer's call stack,
    // which means that `app` may be locked. The event will not actually be dispatched until the
    // full call stack has returned or the current task has yielded to the event loop. It does not
    // guarantee that we will be able to acquire the lock here, but it makes it a lot more likely.
    setTimeout(() => {
      const callbacks = this.#eventMap.get(event);
      if (callbacks) {
        for (const [callback, { once }] of Array.from(callbacks)) {
          callback(...args);
          if (once) callbacks.delete(callback);
        }
      }
    }, 0);
  }

  /**
   * Register an event listener.
   *
   * Returns a function which removes the listener when called.
   *
   * See {@link ViewerEvent} for a full list of available events.
   */
  on<E extends keyof EventsWithValue>(
    event: E,
    callback: (...args: EventsWithValue[E]) => void,
  ): () => void;
  on<E extends keyof EventsWithoutValue>(event: E, callback: () => void): () => void;
  on(event: any, callback: any): () => void {
    const callbacks = this.#eventMap.get(event) ?? new Map();
    callbacks.set(callback, { once: false });
    this.#eventMap.set(event, callbacks);
    return () => callbacks.delete(callback);
  }

  /**
   * Register an event listener which runs only once.
   *
   * Returns a function which removes the listener when called.
   *
   * See {@link ViewerEvent} for a full list of available events.
   */
  once<E extends keyof EventsWithValue>(
    event: E,
    callback: (value: EventsWithValue[E]) => void,
  ): () => void;
  once<E extends keyof EventsWithoutValue>(event: E, callback: () => void): () => void;
  once(event: any, callback: any): () => void {
    const callbacks = this.#eventMap.get(event) ?? new Map();
    callbacks.set(callback, { once: true });
    this.#eventMap.set(event, callbacks);
    return () => callbacks.delete(callback);
  }

  /**
   * Unregister an event listener.
   *
   * The event emitter relies on referential equality to store callbacks.
   * The `callback` passed in must be the exact same _instance_ of the function passed in to `on` or `once`.
   *
   * See {@link ViewerEvent} for a full list of available events.
   */
  off<E extends keyof EventsWithValue>(
    event: E,
    callback: (value: EventsWithValue[E]) => void,
  ): void;
  off<E extends keyof EventsWithoutValue>(event: E, callback: () => void): void;
  off(event: any, callback: any): void {
    const callbacks = this.#eventMap.get(event);
    if (callbacks) {
      callbacks.delete(callback);
    } else {
      console.warn(
        "Attempted to call `WebViewer.off` with an unregistered callback. Are you passing in the same function instance?",
      );
    }
  }

  /**
   * The underlying canvas element.
   */
  get canvas() {
    return this.#canvas;
  }

  /**
   * Returns `true` if the viewer is ready to connect to data sources.
   */
  get ready() {
    return this.#state === "ready";
  }

  /** Start loading from `rrd`, without waiting for it. */
  #addReceivers(rrd: string | string[]) {
    if (!this.#handle) {
      throw new Error(`attempted to open \`${String(rrd)}\` in a stopped viewer`);
    }

    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      try {
        this.#handle.add_receiver(url);
      } catch (e) {
        this.#fail("Failed to open recording", String(e));
        throw e;
      }
    }
  }

  /**
   * Save the active recording as an `.rrd` byte stream.
   *
   * The snapshot is selected when this method is called.
   * Concatenate all stream byte sequences to obtain one valid file.
   * An individual sequence is not independently accepted by {@link WebViewer.openChannel}.
   *
   * @throws If the viewer is stopped or has no active recording.
   */
  saveRecording(): ReadableStream<Uint8Array> {
    if (!this.#handle) {
      throw new Error(`attempted to save recording in a stopped web viewer`);
    }

    const bytes = this.#handle.save_recording();
    return exportStream(bytes);
  }

  /**
   * Save the active blueprint as an `.rbl` byte stream.
   *
   * The snapshot is selected when this method is called.
   * Concatenate all stream byte sequences to obtain one valid file.
   * An individual sequence is not independently accepted by {@link WebViewer.openChannel}.
   *
   * @throws If the viewer is stopped or has no active blueprint.
   */
  saveBlueprint(): ReadableStream<Uint8Array> {
    if (!this.#handle) {
      throw new Error(`attempted to save blueprint in a stopped web viewer`);
    }

    const bytes = this.#handle.save_blueprint();
    return exportStream(bytes);
  }

  /**
   * Stop the viewer, freeing all associated memory.
   *
   * The same viewer instance may be started multiple times.
   */
  stop() {
    if (this.#state === "stopped") return;
    if (this.#allowFullscreen && this.#canvas && this.#fullscreen) {
      this.#minimize();
    }

    this.#state = "stopped";

    this.#canvas?.remove();
    this.#clearLoader();

    try {
      this.#handle?.destroy();
      this.#handle?.free();
    } catch (e) {
      this.#handle = null;
      throw e;
    }

    this.#canvas = null;
    this.#handle = null;
    this.#loader = null;
    this.#fullscreen = false;
    this.#allowFullscreen = false;
  }

  #fail(message: string, errorMessage?: string) {
    console.error("WebViewer failure:", message, errorMessage);
    if (this.canvas?.parentElement) {
      const parent = this.canvas.parentElement;
      parent.innerHTML = `
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: white; font-family: sans-serif; background-color: #1c1c1c;">
          <h1 class="rerun-fail-message"></h1>
          <pre class="rerun-fail-error" style="text-align: left; white-space: pre-wrap; word-break: break-word; max-width: 90vw;"></pre>
          <button class="rerun-fail-clear-cache">Clear caches and reload</button>
        </div>
      `;

      parent.querySelector(".rerun-fail-message")!.textContent = message;

      const errorEl = parent.querySelector(".rerun-fail-error")!;
      if (errorMessage) {
        errorEl.textContent = errorMessage;
      } else {
        errorEl.remove();
      }

      parent.querySelector(".rerun-fail-clear-cache")!.addEventListener("click", async () => {
        if ("caches" in window) {
          const keys = await caches.keys();
          await Promise.all(keys.map((key) => caches.delete(key)));
        }
        window.location.reload();
      });
    }

    this.stop();
  }

  #clearLoader() {
    this.#loader?.remove();
    this.#loader = null;
  }

  /**
   * Opens a new channel for sending log messages.
   *
   * The channel can be used to incrementally push `rrd` chunks into the viewer.
   *
   * @param channelName used to identify the channel.
   */
  openChannel(channelName: string = "rerun-io/web-viewer"): LogChannel {
    if (!this.#handle) {
      throw new Error(`attempted to open channel "${channelName}" in a stopped web viewer`);
    }

    const id = crypto.randomUUID();

    try {
      this.#handle.open_channel(id, channelName);
    } catch (e) {
      this.#fail("Failed to open channel", String(e));
      throw e;
    }

    const onSend = (/** @type {Uint8Array} */ data: Uint8Array) => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel "${channelName}" to a stopped web viewer`,
        );
      }

      try {
        this.#handle.send_rrd_to_channel(id, data);
      } catch (e) {
        this.#fail("Failed to send data", String(e));
        throw e;
      }
    };

    const onSendTable = (/** @type {Uint8Array} */ data: Uint8Array) => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel "${channelName}" to a stopped web viewer`,
        );
      }

      try {
        this.#handle.send_table_to_channel(id, data);
      } catch (e) {
        this.#fail("Failed to send table", String(e));
        throw e;
      }
    };

    const onClose = () => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel "${channelName}" to a stopped web viewer`,
        );
      }

      try {
        this.#handle.close_channel(id);
      } catch (e) {
        this.#fail("Failed to close channel", String(e));
        throw e;
      }
    };

    const getState = () => this.#state;

    return new LogChannel(onSend, onSendTable, onClose, getState);
  }

  /**
   * Force a panel to a specific state.
   *
   * @param panel which panel to configure
   * @param state which state to force the panel into
   */
  overridePanelState(panel: Panel, state: PanelState | undefined | null) {
    if (!this.#handle) {
      throw new Error(`attempted to set ${panel} panel to ${state} in a stopped web viewer`);
    }

    try {
      this.#handle.override_panel_state(panel, state);
    } catch (e) {
      this.#fail("Failed to override panel state", String(e));
      throw e;
    }
  }

  /**
   * Toggle panel overrides set via `overridePanelState`.
   *
   * @param value set to a specific value. Toggles the previous value if not provided.
   */
  togglePanelOverrides(value?: boolean | null) {
    if (!this.#handle) {
      throw new Error(`attempted to toggle panel overrides in a stopped web viewer`);
    }

    try {
      this.#handle.toggle_panel_overrides(value as boolean | undefined);
    } catch (e) {
      this.#fail("Failed to toggle panel overrides", String(e));
      throw e;
    }
  }

  /**
   * Get the active recording id.
   *
   * @deprecated Use the `activeStoreId` of {@link WebViewer.getState} instead.
   */
  "get_active_recording_id"(): string | null {
    if (!this.#handle) {
      throw new Error(`attempted to get active recording id in a stopped web viewer`);
    }

    const storeId = this.#viewerState()?.activeStoreId;
    return storeId === undefined ? null : parseStoreId(storeId).recordingId;
  }

  /**
   * Set the active recording id.
   *
   * This is the same as clicking on the recording in the Viewer's left panel.
   *
   * @deprecated Use {@link WebViewer.setLocation} with the recording's `location` from {@link WebViewer.getState} instead.
   */
  "set_active_recording_id"(value: string) {
    if (!this.#handle) {
      throw new Error(`attempted to set active recording id to ${value} in a stopped web viewer`);
    }

    const recording = this.#recording(value);
    if (recording) {
      this.#logFailure("setLocation", this.setLocation(recording.location));
    }
  }

  /**
   * What the viewer currently shows.
   *
   * Covers the active recording, every open recording with its timelines, their time ranges and
   * its time cursor, and every view of the current blueprint with the warnings and errors it reports.
   *
   * This API is experimental, and may change between releases.
   */
  async getState(): Promise<ViewerState> {
    return viewerStateOf(await this.#viewerControl("getState", {}));
  }

  /**
   * What the user has selected: entities or instances of them, components, views, containers, and recordings.
   *
   * Other things the user can select, such as servers and catalog entries, are left out.
   *
   * This API is experimental, and may change between releases.
   */
  async getSelection(): Promise<Selection> {
    return selectionOf(await this.#viewerControl("getSelection", {}));
  }

  /**
   * Move the time cursor, switch the active timeline, or play, pause, or follow the latest data.
   *
   * ```ts
   * await viewer.setTime({ timeline: "frame", sequence: 42, playState: "playing" });
   * ```
   *
   * Rejects with a {@link ViewerControlError} with the code `INVALID_ARGUMENT` if the recording or
   * timeline doesn't exist, or if `playState` is following and a time is set.
   *
   * This API is experimental, and may change between releases.
   */
  async setTime(options: SetTimeOptions): Promise<void> {
    options = normalizeOptions(options);
    const time = timeOf(options);
    await this.#viewerControl("setTimeCursor", {
      storeId: options.recording,
      timeline: options.timeline === undefined ? undefined : { name: options.timeline },
      time: time === undefined ? undefined : { time },
      playState: playStateToProto(options.playState),
    });
  }

  /**
   * Where the viewer is: a recording, a catalog entry, the settings, …
   *
   * Pass it to {@link WebViewer.setLocation} to come back to it later.
   * This is a Viewer URL, not the address of the web page.
   * A recording is reported as `about:recording?app_id=…&recording_id=…`.
   *
   * This API is experimental, and may change between releases.
   */
  async getLocation(): Promise<string> {
    return (await this.#viewerControl("getLocation", {})).location;
  }

  /**
   * Show a location the viewer already has open.
   *
   * Takes a location from {@link WebViewer.getLocation}, or the `location` of a recording from {@link WebViewer.getState}.
   * This only navigates, and never loads anything: use {@link WebViewer.open} for that.
   *
   * Rejects with a {@link ViewerControlError} with the code `NOT_FOUND` if the viewer does not have it open,
   * and `INVALID_ARGUMENT` if it is not a location.
   *
   * This API is experimental, and may change between releases.
   */
  async setLocation(location: string): Promise<void> {
    await this.#viewerControl("setLocation", { location });
  }

  /**
   * Open URLs: recording or blueprint files, `rerun://` dataset URIs, server or catalog URLs,
   * or links into a recording.
   *
   * A recording it opens becomes the active recording, unless `navigate` is `false`.
   * A location the viewer already has open is not loaded again, only shown.
   * Resolves once the initial load is done, or after about ten seconds, whichever comes first.
   *
   * ```ts
   * const { storeIds } = await viewer.open("https://app.rerun.io/version/0.39.0/examples/dna.rrd");
   * ```
   *
   * Rejects with a {@link ViewerControlError} with the code `NOT_FOUND` for an `about:recording` URL of a recording that is not loaded.
   *
   * This API is experimental, and may change between releases.
   */
  async open(urls: string | string[], options: OpenOptions = {}): Promise<OpenResult> {
    options = normalizeOptions(options);
    const responses = await Promise.all(
      (Array.isArray(urls) ? urls : [urls]).map((url) =>
        this.#viewerControl("open", { url, navigate: options.navigate }),
      ),
    );
    return withLegacyFields({
      storeIds: responses.flatMap((response) => response.storeIds as RecordingStoreId[]),
      loading: responses.flatMap((response) => response.loading),
    });
  }

  /**
   * Close recordings, and return the store ids of the ones that were closed.
   *
   * Closing by URL also stops loading from it.
   * This only removes recordings from the viewer: unsaved blueprint edits are lost.
   *
   * ```ts
   * await viewer.close("https://app.rerun.io/version/0.39.0/examples/dna.rrd");
   * await viewer.close("all");
   * ```
   *
   * Rejects with a {@link ViewerControlError} with the code `NOT_FOUND` if a recording is not open,
   * or nothing is loaded from a URL. Then nothing is closed.
   *
   * This API is experimental, and may change between releases.
   */
  async close(target: CloseTarget): Promise<StoreId[]> {
    target = normalizeOptions(target);
    const { closed } = await this.#viewerControl("close", { target: closeTargetOf(target) });
    return closed as StoreId[];
  }

  /**
   * The viewer's recent log messages, `INFO` and above, oldest first.
   *
   * Pass the `sequence` of the last entry seen to only get what is new.
   *
   * This API is experimental, and may change between releases.
   */
  async getLogs(afterSequence?: bigint): Promise<ViewerLogEntry[]> {
    const { entries } = await this.#viewerControl("getLogs", {
      afterSequence,
    });
    return entries.map(({ sequence, level, target, message }) => ({
      sequence,
      level: level as ViewerLogLevel,
      target,
      message,
    }));
  }

  /**
   * A recording's timelines, entities, and the components logged on them.
   *
   * Use `pathsOnly` to see the shape of a large recording,
   * then `entityPath` for the components of the entities that matter.
   *
   * Rejects with a {@link ViewerControlError} with the code `NOT_FOUND` if the recording is not open,
   * or nothing in it is at or below `entityPath`.
   *
   * This API is experimental, and may change between releases.
   */
  async getRecordingSchema(options: RecordingSchemaOptions = {}): Promise<RecordingSchema> {
    options = normalizeOptions(options);
    const response = await this.#viewerControl("getRecordingSchema", {
      storeId: options.recording,
      entityPath: options.entityPath,
      maxEntities: options.maxEntities,
      withArchetypes: false,
      withComponents: !options.pathsOnly,
    });
    return withLegacyFields({
      recording: response.storeId as RecordingStoreId,
      timelines: response.timelines.map(timelineOf),
      entities: response.entities.map(entitySchemaOf),
      omittedEntities: response.omittedEntities,
    });
  }

  /**
   * The entities of a dataset's segments on a server, and the components logged on them.
   *
   * Nothing is opened or loaded.
   *
   * Rejects with a {@link ViewerControlError} with the code `FAILED_PRECONDITION` if no `datasetUrl` is given
   * and no dataset is on screen, and `NOT_FOUND` if the viewer does not know the dataset.
   *
   * This API is experimental, and may change between releases.
   */
  async getDatasetSchema(options: DatasetSchemaOptions = {}): Promise<DatasetSchema> {
    options = normalizeOptions(options);
    const response = await this.#viewerControl("getDatasetSchema", {
      datasetUrl: options.datasetUrl,
      ...schemaRequestOf(options),
    });
    return withLegacyFields({
      timelines: response.timelines,
      entities: response.entities.map(entitySchemaOf),
      omittedEntities: response.omittedEntities,
      assetIds: response.assetIds,
    });
  }

  /**
   * The assets of a dataset on a server, and the entities logged in each.
   *
   * Rejects like {@link WebViewer.getDatasetSchema}.
   *
   * This API is experimental, and may change between releases.
   */
  async getDatasetAssetsSchema(options: DatasetSchemaOptions = {}): Promise<DatasetAssetSchema[]> {
    options = normalizeOptions(options);
    const { assets } = await this.#viewerControl("getDatasetAssetsSchema", {
      datasetUrl: options.datasetUrl,
      ...schemaRequestOf(options),
    });
    return assets.map((asset) =>
      withLegacyFields({
        assetId: asset.assetId,
        entities: asset.entities.map(entitySchemaOf),
        omittedEntities: asset.omittedEntities,
      }),
    );
  }

  /**
   * A table's columns in display order, hidden ones included.
   *
   * Takes a dataset or table `url` from the `servers` of {@link WebViewer.getState}.
   * If omitted, the table on screen.
   *
   * This API is experimental, and may change between releases.
   */
  async getTableSchema(tableUrl?: string): Promise<TableColumnSchema[]> {
    const { columns } = await this.#viewerControl("getTableSchema", { tableUrl });
    return columns.map(({ name, displayName, datatype }) =>
      withLegacyFields({
        name,
        displayName,
        datatype,
      }),
    );
  }

  /**
   * Read a blueprint.
   *
   * If `target` is omitted, reads the blueprint of what is on screen.
   * A recording or table without a blueprint gets a default one.
   *
   * This API is experimental, and may change between releases.
   */
  async getBlueprint(target?: BlueprintTarget): Promise<Blueprint> {
    target = normalizeOptions(target);
    const response = await this.#viewerControl("getBlueprint", {
      target: target === undefined ? undefined : blueprintTargetOf(target),
    });
    return withLegacyFields({
      blueprintId: response.blueprintId as BlueprintStoreId,
      blueprint: parseExactJson(response.json) as BlueprintJson,
      recording: response.storeId as RecordingStoreId,
      tableUrl: response.tableUrl,
    });
  }

  /**
   * Replace a whole blueprint, in the form {@link WebViewer.getBlueprint} reads it.
   *
   * Anything the blueprint leaves out is cleared, so read it first, and change what you need.
   *
   * Rejects with a {@link ViewerControlError} with the code `INVALID_ARGUMENT` that says where the blueprint is wrong.
   *
   * This API is experimental, and may change between releases.
   */
  async setBlueprint(blueprint: BlueprintJson, target: BlueprintTarget): Promise<BlueprintRef> {
    target = normalizeOptions(target);
    const response = await this.#viewerControl("setBlueprint", {
      json: stringifyExactJson(blueprint),
      target: blueprintTargetOf(target),
    });
    return withLegacyFields({
      blueprintId: response.blueprintId as BlueprintStoreId,
      recording: response.storeId as RecordingStoreId,
      tableUrl: response.tableUrl,
    });
  }

  /**
   * A recording's transform graph at its time cursor: its frames, and the edges between them.
   * See [Transforms & Coordinate Frames](https://rerun.io/docs/logging-and-ingestion/data-model/transforms).
   *
   * This API is experimental, and may change between releases.
   */
  async getTransformGraph(options: TransformGraphOptions = {}): Promise<TransformGraph> {
    options = normalizeOptions(options);
    const response = await this.#viewerControl("getTransformGraph", {
      storeId: options.recording,
      frameFilter: options.frameFilter && FRAME_FILTERS[options.frameFilter],
      edgeFilter: options.edgeFilter && EDGE_FILTERS[options.edgeFilter],
      includeTransforms: options.includeTransforms,
    });
    return {
      recording: response.storeId as RecordingStoreId,
      graph: parseExactJson(response.json) as Record<string, unknown>,
    };
  }

  /**
   * Take a screenshot of the viewer, or of one of its views, as a PNG.
   *
   * Rejects with a {@link ViewerControlError} with the code `INVALID_ARGUMENT` if `viewId` is not a UUID,
   * `NOT_FOUND` if no such view is visible, and `FAILED_PRECONDITION` if the view is too small to capture.
   *
   * This API is experimental, and may change between releases.
   */
  async takeScreenshot(options: ScreenshotOptions = {}): Promise<Uint8Array> {
    options = normalizeOptions(options);
    const { png } = await this.#viewerControl("saveScreenshot", {
      viewId: options.viewId,
      maxSize: options.maxSize,
    });
    if (png === undefined) {
      throw new Error("Expected the screenshot as a PNG");
    }
    return png;
  }

  /**
   * Draw a pulsing outline around a rectangle, to point the user at something.
   *
   * It pulses until the user clicks. Pass `null` to dismiss it.
   *
   * This API is experimental, and may change between releases.
   */
  async highlightRect(rect: ScreenRect | null, label?: string): Promise<void> {
    await this.#viewerControl("highlightRect", { rect: rect ?? undefined, label });
  }

  /**
   * The command palette commands, sorted by id, or the ones named in `ids`, in that order.
   *
   * Rejects with a {@link ViewerControlError} with the code `NOT_FOUND` if an id in `ids` is unknown.
   *
   * This API is experimental, and may change between releases.
   */
  async listCommands(options: ListCommandsOptions = {}): Promise<ViewerCommand[]> {
    options = normalizeOptions(options);
    const target = { storeId: options.recording, serverOrigin: options.serverOrigin };
    const ids =
      options.ids ??
      (
        await this.#viewerControl("listCommands", {
          includeUnavailable: options.includeUnavailable,
          ...target,
        })
      ).commands.map(({ id }) => id);
    const { commands } = await this.#viewerControl("describeCommands", { ids, ...target });
    return commands.map(viewerCommandOf);
  }

  /**
   * List supported view types before choosing a class for a blueprint. No recording is required.
   *
   * This API is experimental, and may change between releases.
   */
  async listViewTypes(): Promise<ViewerViewType[]> {
    const { viewTypes } = await this.#viewerControl("listViewTypes", {});
    return viewTypes.map((view) => ({
      classIdentifier: view.classIdentifier,
      displayName: view.displayName,
      description: view.description,
      keywords: view.keywords,
    }));
  }

  /**
   * Run a command palette command by id, and return the command.
   *
   * Resolves once the command is queued, not when it is done.
   *
   * This API is experimental, and may change between releases.
   */
  async runCommand(id: string, options: RunCommandOptions = {}): Promise<ViewerCommand> {
    options = normalizeOptions(options);
    const { command } = await this.#viewerControl("runCommand", {
      id,
      storeId: options.recording,
      serverOrigin: options.serverOrigin,
      allowBlockingNativeModal: options.allowBlockingNativeModal,
    });
    if (command === undefined) {
      throw new Error("Expected the command that ran");
    }
    return viewerCommandOf(command);
  }

  /**
   * Get the play state.
   *
   * This always returns `false` if the recording can't be found.
   *
   * @deprecated Use the recording's `playState` from {@link WebViewer.getState} instead.
   */
  "get_playing"(recordingId: string): boolean {
    if (!this.#handle) {
      throw new Error(`attempted to get play state in a stopped web viewer`);
    }

    return this.#recording(recordingId)?.playState === "playing";
  }

  /**
   * Set the play state.
   *
   * This does nothing if the recording can't be found.
   *
   * @deprecated Use {@link WebViewer.setTime} with `playState` instead.
   */
  "set_playing"(recordingId: string, value: boolean) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set play state to ${value ? "playing" : "paused"} in a stopped web viewer`,
      );
    }

    const recording = this.#recording(recordingId);
    if (recording) {
      this.#logFailure(
        "setTime",
        this.setTime({ recording: recording.storeId, playState: value ? "playing" : "paused" }),
      );
    }
  }

  /**
   * Get the current time.
   *
   * The interpretation of time depends on what kind of timeline it is:
   *
   * - For time timelines, this is the time in nanoseconds.
   * - For sequence timelines, this is the sequence number.
   *
   * This always returns `0` if the recording or timeline can't be found.
   * The time is a whole number.
   *
   * @deprecated Use the timeline's `currentTime` from {@link WebViewer.getState} instead.
   */
  "get_current_time"(recordingId: string, timeline: string): number {
    if (!this.#handle) {
      throw new Error(`attempted to get current time in a stopped web viewer`);
    }

    return Number(this.#timeline(recordingId, timeline)?.currentTime ?? 0);
  }

  /**
   * Set the current time.
   *
   * Equivalent to clicking on the timeline in the time panel at the specified `time`.
   * The interpretation of `time` depends on what kind of timeline it is:
   *
   * - For time timelines, this is the time in nanoseconds.
   * - For sequence timelines, this is the sequence number.
   *
   * This does nothing if the recording or timeline can't be found.
   * A fractional time is rounded down.
   *
   * @deprecated Use {@link WebViewer.setTime} instead.
   */
  "set_current_time"(recordingId: string, timeline: string, time: number) {
    if (!this.#handle) {
      throw new Error(`attempted to set current time to ${time} in a stopped web viewer`);
    }

    const recording = this.#recording(recordingId);
    if (recording && Number.isFinite(time)) {
      const value = BigInt(Math.floor(time));
      const timeType = recording.timelines.find((t) => t.name === timeline)?.timeType;
      const at: TimeValue =
        timeType === "timestamp"
          ? { timestampNanos: value }
          : timeType === "duration"
            ? { durationNanos: value }
            : { sequence: value };
      this.#logFailure("setTime", this.setTime({ recording: recording.storeId, timeline, ...at }));
    }
  }

  /**
   * Get the active timeline.
   *
   * This always returns `null` if the recording can't be found.
   *
   * @deprecated Use the recording's `currentTime.timeline` from {@link WebViewer.getState} instead.
   */
  "get_active_timeline"(recordingId: string): string | null {
    if (!this.#handle) {
      throw new Error(`attempted to get active timeline in a stopped web viewer`);
    }

    return this.#recording(recordingId)?.currentTime?.timeline ?? null;
  }

  /**
   * Set the active timeline.
   *
   * This does nothing if the recording or timeline can't be found.
   *
   * @deprecated Use {@link WebViewer.setTime} with `timeline` instead.
   */
  "set_active_timeline"(recordingId: string, timeline: string) {
    if (!this.#handle) {
      throw new Error(`attempted to set active timeline to ${timeline} in a stopped web viewer`);
    }

    const recording = this.#recording(recordingId);
    if (recording) {
      this.#logFailure("setTime", this.setTime({ recording: recording.storeId, timeline }));
    }
  }

  /**
   * Get the time range for a timeline.
   *
   * This always returns `null` if the recording or timeline can't be found.
   *
   * @deprecated Use the timeline's `timeRange` from {@link WebViewer.getState} instead.
   */
  "get_time_range"(recordingId: string, timeline: string): { min: number; max: number } | null {
    if (!this.#handle) {
      throw new Error(`attempted to get time range in a stopped web viewer`);
    }

    const range = this.#timeline(recordingId, timeline)?.timeRange;
    return range ? { min: Number(range.start), max: Number(range.end) } : null;
  }

  /**
   * The viewer state, read right away, for the deprecated synchronous getters.
   *
   * Read at most once per task, since getters are called in bursts and a read is not cheap.
   * `null` while the viewer is busy painting a frame, when the old getters found nothing either.
   */
  #viewerState(): ViewerState | null {
    if (!this.#handle) {
      throw new Error(`attempted to get the viewer state of a stopped web viewer`);
    }
    if (this.#cachedViewerState === null) {
      try {
        this.#cachedViewerState = viewerStateOf(
          fromBinary(GetStateResponseSchema, this.#handle.viewer_state()),
        );
      } catch {
        return null;
      }
      queueMicrotask(() => {
        this.#cachedViewerState = null;
      });
    }
    return this.#cachedViewerState;
  }

  /** The open recording with this recording id, if any. */
  #recording(recordingId: string): ViewerRecording | undefined {
    return this.#viewerState()?.recordings.find(
      (recording) => parseStoreId(recording.storeId).recordingId === recordingId,
    );
  }

  /** The timeline of the open recording with this recording id, if any. */
  #timeline(recordingId: string, timeline: string): ViewerTimeline | undefined {
    return this.#recording(recordingId)?.timelines.find((candidate) => candidate.name === timeline);
  }

  /** Run what a deprecated setter started, which has no way to report a failure, and log one instead. */
  #logFailure(method: string, call: Promise<void>) {
    call.catch((err) => {
      console.warn(`WebViewer.${method} failed:`, err);
    });
  }

  /**
   * Toggle fullscreen mode.
   *
   * This does nothing if `allowFullscreen` was not set to `true` when starting the viewer.
   *
   * Fullscreen mode works by updating the underlying `<canvas>` element's `style`:
   * - `position` to `fixed`
   * - width/height/top/left to cover the entire viewport
   *
   * When fullscreen mode is toggled off, the style is restored to its previous values.
   *
   * When fullscreen mode is toggled on, any other instance of the viewer on the page
   * which is already in fullscreen mode is toggled off. This means that it doesn't
   * have to be tracked manually.
   *
   * This functionality can also be directly accessed in the viewer:
   * - The maximize/minimize top panel button
   * - The `Toggle fullscreen` UI command (accessible via the command palette, CTRL+P)
   */
  toggleFullscreen() {
    if (!this.#allowFullscreen) return;

    if (!this.#handle || !this.#canvas) {
      throw new Error(`attempted to toggle fullscreen mode in a stopped web viewer`);
    }

    if (this.#fullscreen) {
      this.#minimize();
    } else {
      this.#maximize();
    }
  }

  setCredentials(accessToken: string, email: string) {
    if (!this.#handle) {
      throw new Error(`attempted to set credentials in a stopped web viewer`);
    }
    this.#handle.set_credentials(accessToken, email);
  }

  #minimize = () => {};

  #maximize = () => {
    minimizeCurrentFullscreenViewer?.();

    const canvas = this.#canvas!;
    const rect = canvas.getBoundingClientRect();

    const syncStyleToRect = () => {
      canvas.style.left = rect.left + "px";
      canvas.style.top = rect.top + "px";
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
    };
    const undoStyle = () => canvas.removeAttribute("style");
    const transition = (callback: () => void) =>
      setTimeout(() => requestAnimationFrame(callback), transitionDelayMs);

    canvas.classList.add(classes.fullscreenBase, classes.fullscreenRect);
    syncStyleToRect();
    requestAnimationFrame(() => {
      if (!this.#fullscreen) return;
      canvas.classList.add(classes.transition);
      transition(() => {
        if (!this.#fullscreen) return;
        undoStyle();

        document.body.classList.add(classes.hideScrollbars);
        document.documentElement.classList.add(classes.hideScrollbars);
        this.#dispatchEvent("fullscreen", true);
      });
    });

    this.#minimize = () => {
      document.body.classList.remove(classes.hideScrollbars);
      document.documentElement.classList.remove(classes.hideScrollbars);

      syncStyleToRect();
      canvas.classList.remove(classes.fullscreenRect);
      transition(() => {
        if (this.#fullscreen) return;

        undoStyle();
        canvas.classList.remove(classes.fullscreenBase, classes.transition);
      });

      minimizeCurrentFullscreenViewer = null;
      this.#fullscreen = false;
      this.#dispatchEvent("fullscreen", false);
    };

    minimizeCurrentFullscreenViewer = () => this.#minimize();
    this.#fullscreen = true;
  };

  /**
   * Perform one operation of the viewer's control protocol, defined by `viewer_control.proto`.
   *
   * A failed operation rejects with a {@link ViewerControlError}.
   */
  async #viewerControl<K extends Operation>(
    operation: K,
    request: OperationRequest<K>,
  ): Promise<OperationResponse<K>> {
    if (!this.#handle) {
      throw new ViewerControlError(
        "FAILED_PRECONDITION",
        "attempted to control a stopped web viewer",
      );
    }

    const kind = { case: operation, value: request } as MessageInitShape<
      typeof ViewerControlRequestSchema
    >["kind"];
    const requestBytes = toBinary(
      ViewerControlRequestSchema,
      create(ViewerControlRequestSchema, { kind }),
    );
    let responseBytes;
    try {
      responseBytes = await this.#handle.viewer_control(requestBytes);
    } catch (err) {
      throw asViewerControlError(err);
    }
    const response = fromBinary(ViewerControlResponseSchema, responseBytes);

    if (response.kind.case !== operation) {
      throw new Error(`Expected a ${operation} response, got ${response.kind.case}`);
    }
    return response.kind.value as OperationResponse<K>;
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.saveRecording} instead. */
  "save_recording"() {
    return this.saveRecording();
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.saveBlueprint} instead. */
  "save_blueprint"() {
    return this.saveBlueprint();
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.openChannel} instead. */
  "open_channel"(channelName: string = "rerun-io/web-viewer") {
    return this.openChannel(channelName);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.overridePanelState} instead. */
  "override_panel_state"(panel: Panel, state: PanelState | undefined | null) {
    return this.overridePanelState(panel, state);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.togglePanelOverrides} instead. */
  "toggle_panel_overrides"(value?: boolean | null) {
    return this.togglePanelOverrides(value);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getState} instead. */
  "get_state"() {
    return this.getState();
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.setTime} instead. */
  "set_time"(options: SetTimeOptions) {
    return this.setTime(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getLocation} instead. */
  "get_location"() {
    return this.getLocation();
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.setLocation} instead. */
  "set_location"(location: string) {
    return this.setLocation(location);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getLogs} instead. */
  "get_logs"(afterSequence?: bigint) {
    return this.getLogs(afterSequence);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getRecordingSchema} instead. */
  "get_recording_schema"(options: RecordingSchemaOptions = {}) {
    return this.getRecordingSchema(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getDatasetSchema} instead. */
  "get_dataset_schema"(options: DatasetSchemaOptions = {}) {
    return this.getDatasetSchema(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getDatasetAssetsSchema} instead. */
  "get_dataset_assets_schema"(options: DatasetSchemaOptions = {}) {
    return this.getDatasetAssetsSchema(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getTableSchema} instead. */
  "get_table_schema"(tableUrl?: string) {
    return this.getTableSchema(tableUrl);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getBlueprint} instead. */
  "get_blueprint"(target?: BlueprintTarget) {
    return this.getBlueprint(target);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.setBlueprint} instead. */
  "set_blueprint"(blueprint: BlueprintJson, target: BlueprintTarget) {
    return this.setBlueprint(blueprint, target);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.getTransformGraph} instead. */
  "get_transform_graph"(options: TransformGraphOptions = {}) {
    return this.getTransformGraph(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.takeScreenshot} instead. */
  "take_screenshot"(options: ScreenshotOptions = {}) {
    return this.takeScreenshot(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.highlightRect} instead. */
  "highlight_rect"(rect: ScreenRect | null, label?: string) {
    return this.highlightRect(rect, label);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.listCommands} instead. */
  "list_commands"(options: ListCommandsOptions = {}) {
    return this.listCommands(options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.runCommand} instead. */
  "run_command"(id: string, options: RunCommandOptions = {}) {
    return this.runCommand(id, options);
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.toggleFullscreen} instead. */
  "toggle_fullscreen"() {
    return this.toggleFullscreen();
  }

  /** @deprecated Since 0.43.0. Use {@link WebViewer.setCredentials} instead. */
  "set_credentials"(accessToken: string, email: string) {
    return this.setCredentials(accessToken, email);
  }
}

export class LogChannel {
  #onSend;
  #onSendTable;
  #onClose;
  #getState;
  #closed = false;

  /**
   * @param onSend
   * @param onClose
   * @param getState
   */
  constructor(
    onSend: (data: Uint8Array) => void,
    onSendTable: (data: Uint8Array) => void,
    onClose: () => void,
    getState: () => "ready" | "starting" | "stopped",
  ) {
    this.#onSend = onSend;
    this.#onSendTable = onSendTable;
    this.#onClose = onClose;
    this.#getState = getState;
  }

  get ready() {
    return !this.#closed && this.#getState() === "ready";
  }

  /**
   * Send an `rrd` containing log messages to the viewer.
   *
   * Does nothing if `!this.ready`.
   *
   * @param rrdBytes Is an rrd file stored in a byte array, received via some other side channel.
   */
  sendRrd(rrdBytes: Uint8Array) {
    if (!this.ready) return;
    this.#onSend(rrdBytes);
  }

  sendTable(tableBytes: Uint8Array) {
    if (!this.ready) return;
    this.#onSendTable(tableBytes);
  }

  /**
   * Close the channel.
   *
   * Does nothing if `!this.ready`.
   */
  close() {
    if (!this.ready) return;
    this.#onClose();
    this.#closed = true;
  }

  /** @deprecated Since 0.43.0. Use {@link LogChannel.sendRrd} instead. */
  "send_rrd"(rrdBytes: Uint8Array) {
    return this.sendRrd(rrdBytes);
  }

  /** @deprecated Since 0.43.0. Use {@link LogChannel.sendTable} instead. */
  "send_table"(tableBytes: Uint8Array) {
    return this.sendTable(tableBytes);
  }
}

const classes = {
  hideScrollbars: "rerun-viewer-hide-scrollbars",
  fullscreenBase: "rerun-viewer-fullscreen-base",
  fullscreenRect: "rerun-viewer-fullscreen-rect",
  transition: "rerun-viewer-transition",
};

const transitionDelayMs = 100;

const css = `
  html.${classes.hideScrollbars},
  body.${classes.hideScrollbars} {
    scrollbar-gutter: auto !important;
    overflow: hidden !important;
  }

  .${classes.fullscreenBase} {
    position: fixed;
    z-index: 99999;
  }

  .${classes.transition} {
    transition: all ${transitionDelayMs / 1000}s linear;
  }

  .${classes.fullscreenRect} {
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
  }
`;

function injectStyle() {
  const ID = "__rerun_viewer_style";

  if (document.getElementById(ID)) {
    // already injected
    return;
  }

  const style = document.createElement("style");
  style.id = ID;
  style.appendChild(document.createTextNode(css));
  document.head.appendChild(style);
}

function setupGlobalEventListeners() {
  window.addEventListener("keyup", (e) => {
    if (e.code === "Escape") {
      minimizeCurrentFullscreenViewer?.();
    }
  });
}
