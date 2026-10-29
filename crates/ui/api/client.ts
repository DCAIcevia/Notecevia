// 对 Tauri 命令的薄封装：只做参数与错误的规整，不做业务。

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppConfigSnapshot,
  AppInfo,
  BoardSummary,
  DocumentDto,
  IpcFailure,
  SaveResult,
  SearchHit,
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
  openBoard: (boardId: number) => call<DocumentDto>("open_board", { boardId }),
  createBoard: (title: string) => call<BoardSummary>("create_board", { title }),
  renameBoard: (boardId: number, title: string) => call<null>("rename_board", { boardId, title }),
  deleteBoard: (boardId: number) => call<null>("delete_board", { boardId }),
  searchBoards: (query: string) => call<SearchHit[]>("search_boards", { query }),
  saveBoard: (boardId: number, document: DocumentDto) =>
    call<SaveResult>("save_board", { boardId, document }),
  readMedia: (hash: string) => call<ArrayBuffer>("read_media", { hash }),
  writeMedia: (name: string, data: number[]) => call<string>("write_media", { name, data }),
};