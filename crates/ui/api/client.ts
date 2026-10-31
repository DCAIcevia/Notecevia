// 对 Tauri 命令的薄封装：只做参数与错误的规整，不做业务。

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppConfigSnapshot,
  AppInfo,
  BoardSummary,
  DocumentDto,
  IpcFailure,
  PageCreated,
  SaveResult,
  SearchHit,
  ShelfDto,
  ShelfSummary,
  TrashItem,
  VolumeDto,
  VolumeSummary,
  WorkspaceStatus,
} from "./types";

type Invoker = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

// 开发期可以直接看界面：地址带 ?mock 时改用假数据，生产构建里这段不会被打进去。
const useMock = import.meta.env.DEV && new URLSearchParams(window.location.search).has("mock");

let mockInvoker: Promise<Invoker> | null = null;

function invoker(): Invoker | Promise<Invoker> {
  if (!useMock) return tauriInvoke as Invoker;
  mockInvoker ??= import("./mock").then((module) => module.mockInvoke as Invoker);
  return mockInvoker;
}

function normalizeError(error: unknown): IpcFailure {
  if (typeof error === "string") {
    return { kind: "internal", message: error };
  }
  if (error && typeof error === "object" && "kind" in error && "message" in error) {
    const failure = error as IpcFailure;
    return { kind: failure.kind, message: failure.message };
  }
  return { kind: "internal", message: String(error) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    const invoke = await invoker();
    return (await invoke(command, args)) as T;
  } catch (error) {
    throw normalizeError(error);
  }
}

export const api = {
  appConfig: () => call<AppConfigSnapshot>("app_config"),
  saveAppConfig: (config: AppConfig) => call<null>("save_app_config", { config }),
  appInfo: () => call<AppInfo>("app_info"),
  openWorkspace: (path?: string) =>
    call<WorkspaceStatus>("open_workspace", path === undefined ? undefined : { path }),
  workspaceStatus: () => call<WorkspaceStatus | null>("workspace_status"),
  listBoards: () => call<BoardSummary[]>("list_boards"),
  rebuildIndex: () => call<number>("rebuild_index"),
  exportContainer: (boardId: number) => call<string>("export_container", { boardId }),
  exportHtml: (boardId: number) => call<string>("export_html", { boardId }),
  saveThumbnail: (boardId: number, png: number[]) => call<null>("save_thumbnail", { boardId, png }),
  openBoard: (boardId: number) => call<DocumentDto>("open_board", { boardId }),
  createBoard: (title: string) => call<BoardSummary>("create_board", { title }),
  renameBoard: (boardId: number, title: string) => call<null>("rename_board", { boardId, title }),
  deleteBoard: (boardId: number) => call<null>("delete_board", { boardId }),
  searchBoards: (query: string) => call<SearchHit[]>("search_boards", { query }),
  saveBoard: (boardId: number, document: DocumentDto) =>
    call<SaveResult>("save_board", { boardId, document }),
  readMedia: (hash: string) => call<ArrayBuffer>("read_media", { hash }),
  readThumbnail: (boardId: number) => call<ArrayBuffer>("read_thumbnail", { boardId }),
  writeMedia: (name: string, data: number[]) => call<string>("write_media", { name, data }),
  listVolumes: () => call<VolumeSummary[]>("list_volumes"),
  loadVolume: (volumeId: number) => call<VolumeDto>("load_volume", { volumeId }),
  saveVolume: (volumeId: number, volume: VolumeDto) => call<null>("save_volume", { volumeId, volume }),
  createVolume: (title: string) => call<VolumeSummary>("create_volume", { title }),
  createPage: (title: string, volumeId?: number) =>
    call<PageCreated>("create_page", volumeId === undefined ? { title } : { title, volumeId }),
  deleteVolume: (volumeId: number) => call<null>("delete_volume", { volumeId }),
  assignPage: (volumeId: number, page: number, at: number) =>
    call<number | null>("assign_page", { volumeId, page, at }),
  unassignPage: (page: number) => call<number | null>("unassign_page", { page }),
  movePageInVolume: (volumeId: number, from: number, to: number) =>
    call<null>("move_page_in_volume", { volumeId, from, to }),
  pagesInVolume: (volumeId: number) => call<number[]>("pages_in_volume", { volumeId }),
  listShelves: () => call<ShelfSummary[]>("list_shelves"),
  loadShelf: (shelfId: number) => call<ShelfDto>("load_shelf", { shelfId }),
  saveShelf: (shelfId: number, shelf: ShelfDto) => call<null>("save_shelf", { shelfId, shelf }),
  createShelf: (title: string) => call<ShelfSummary>("create_shelf", { title }),
  deleteShelf: (shelfId: number) => call<null>("delete_shelf", { shelfId }),
  shelveVolume: (shelfId: number, volumeId: number) =>
    call<null>("shelve_volume", { shelfId, volumeId }),
  unshelveVolume: (shelfId: number, volumeId: number) =>
    call<null>("unshelve_volume", { shelfId, volumeId }),
  createShelfWith: (title: string, volumes: number[]) =>
    call<ShelfSummary>("create_shelf_with", { title, volumes }),
  listTrash: () => call<TrashItem[]>("list_trash"),
  restoreTrash: (folder: string) => call<null>("restore_trash", { folder }),
  purgeTrashItem: (folder: string) => call<null>("purge_trash_item", { folder }),
  emptyTrash: () => call<number>("empty_trash"),
};