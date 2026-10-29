// 工作区状态：打开一次，列表与画板都从它拿。

import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "../api/client";
import type { BoardSummary, IpcFailure, WorkspaceStatus } from "../api/types";

export const useWorkspaceStore = defineStore("workspace", () => {
  const status = ref<WorkspaceStatus | null>(null);
  const boards = ref<BoardSummary[]>([]);
  const boardsLoading = ref(false);
  const boardsError = ref<IpcFailure | null>(null);

  async function open(): Promise<void> {
    const info = await api.appInfo();
    status.value = await api.openWorkspace(info.defaultWorkspace);
  }

  async function refreshBoards(): Promise<void> {
    boardsLoading.value = true;
    boardsError.value = null;
    try {
      boards.value = await api.listBoards();
    } catch (error) {
      boardsError.value = error as IpcFailure;
    } finally {
      boardsLoading.value = false;
    }
  }

  return { status, boards, boardsLoading, boardsError, open, refreshBoards };
});
