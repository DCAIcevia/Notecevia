// 窗口壳 IPC 的数据形状。字段与 docs/design/ipc.md 保持一一对应。

export type Rgb = [number, number, number];

export interface Geometry {
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
}

export type BlockKind = "text" | "image" | "video" | "audio" | "table" | "shape" | "highlight";

export interface Entry {
  id: number;
  kind: BlockKind;
  geometry: Geometry;
  z: number;
  locked: boolean;
  visible: boolean;
}

export interface TextBody {
  kind: "text";
  id: number;
  text: string;
  font?: string | null;
  size?: number | null;
  color?: Rgb | null;
  background?: Rgb | null;
}

export interface ImageBody {
  kind: "image";
  id: number;
  media: string;
  name: string;
  fit: "cover" | "contain" | "fill";
}

export interface MediaBody {
  kind: "media";
  id: number;
  media: string;
  name: string;
  bytes: number;
}

export interface TableBody {
  kind: "table";
  id: number;
  rows: number;
  cols: number;
  cells: string[];
}

export interface ShapeBody {
  kind: "shape";
  id: number;
  shape: "rect" | "ellipse" | "line";
  fill?: Rgb | null;
  stroke?: Rgb | null;
  strokeWidth: number;
}

export interface HighlightBody {
  kind: "highlight";
  id: number;
  color: Rgb;
  opacity: number;
}

export type Body = TextBody | ImageBody | MediaBody | TableBody | ShapeBody | HighlightBody;

export interface BoardMeta {
  title: string;
  tags: string[];
}

export interface DocumentDto {
  board: BoardMeta;
  entries: Entry[];
  bodies: Body[];
}

export interface BoardSummary {
  id: number;
  title: string;
  tags: string[];
  updatedAt: number;
  entryCount: number;
}

export interface WorkspaceStatus {
  root: string;
  purgedOnOpen: number;
}

export interface AppInfo {
  version: string;
  defaultWorkspace: string;
}

export interface SaveResult {
  updatedAt: number;
}

export interface IpcFailure {
  kind: string;
  message: string;
}
export interface AutosavePolicy {
  intervalMs: number;
  thumbnailIntervalMs: number;
}

export interface AppConfig {
  autosave: AutosavePolicy;
}

export interface AppConfigSnapshot {
  path: string;
  config: AppConfig;
}
export interface SearchHit {
  boardId: number;
  title: string;
  snippet: string;
}
export interface VolumeSummary {
  id: number;
  title: string;
  pageCount: number;
  updatedAt: number;
}

export interface VolumeDto {
  title: string;
  pages: number[];
}

export interface PageCreated {
  board: BoardSummary;
  volume: VolumeSummary;
}

export interface ShelfSummary {
  id: number;
  title: string;
  volumeCount: number;
  updatedAt: number;
}

export interface ShelfDto {
  title: string;
  volumes: number[];
}

export interface TrashItem {
  folder: string;
  kind: "board" | "volume" | "shelf";
  id: number;
  title: string;
  deletedAt: number;
  bytes: number;
  daysLeft: number;
}
