<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import dayjs from "dayjs";
import BaseButton from "../components/base/BaseButton.vue";
import BaseEmptyState from "../components/base/BaseEmptyState.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import { api } from "../api/client";
import { useWorkspaceStore } from "../stores/workspace";

const workspace = useWorkspaceStore();
const router = useRouter();

const bootError = ref<string | null>(null);
const creating = ref(false);

async function boot(): Promise<void> {
  bootError.value = null;
  try {
    if (!workspace.status) await workspace.open();
    await workspace.refreshBoards();
  } catch (error) {
    bootError.value = (error as { message?: string }).message ?? "打开工作区失败";
  }
}

onMounted(boot);

function formatTime(seconds: number): string {
  return dayjs(seconds * 1000).format("YYYY-MM-DD HH:mm");
}

async function createBoard(): Promise<void> {
  creating.value = true;
  try {
    const board = await api.createBoard("未命名画板");
    router.push({ name: "board", params: { id: board.id } });
  } finally {
    creating.value = false;
  }
}

function openBoard(id: number): void {
  router.push({ name: "board", params: { id } });
}
</script>

<template>
  <main class="home">
    <header class="home__bar">
      <h1 class="home__title">画板</h1>
      <BaseButton :disabled="creating" @click="createBoard">新建画板</BaseButton>
    </header>

    <div v-if="bootError" class="home__state">
      <p class="home__error">{{ bootError }}</p>
      <BaseButton size="sm" @click="boot">重试</BaseButton>
    </div>

    <div v-else-if="workspace.boardsLoading && workspace.boards.length === 0" class="home__state">
      <BaseSpinner />
    </div>

    <div v-else-if="workspace.boardsError" class="home__state">
      <p class="home__error">{{ workspace.boardsError.message }}</p>
      <BaseButton size="sm" @click="workspace.refreshBoards()">重试</BaseButton>
    </div>

    <BaseEmptyState
      v-else-if="workspace.boards.length === 0"
      title="还没有画板"
      hint="画板是笔记的容器，先建一张。"
    >
      <template #action>
        <BaseButton variant="primary" @click="createBoard">新建画板</BaseButton>
      </template>
    </BaseEmptyState>

    <ul v-else class="home__list">
      <li v-for="board in workspace.boards" :key="board.id">
        <button class="row" type="button" @click="openBoard(board.id)">
          <span class="row__title">{{ board.title }}</span>
          <span class="row__meta">{{ board.entryCount }} 块 · {{ formatTime(board.updatedAt) }}</span>
        </button>
      </li>
    </ul>
  </main>
</template>

<style scoped>
.home {
  height: 100%;
  overflow: auto;
  padding: var(--space-6) var(--space-8);
  background: var(--color-surface-app);
}

.home__bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  max-width: 720px;
  margin: 0 auto var(--space-4);
}

.home__title {
  font-size: var(--text-xl);
  font-weight: 600;
  line-height: var(--leading-tight);
}

.home__state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-12) 0;
}

.home__error {
  color: var(--color-danger);
}

.home__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-width: 720px;
  margin: 0 auto;
  padding: 0;
  list-style: none;
}

.row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-4);
  width: 100%;
  padding: var(--space-3) var(--space-3);
  border: none;
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-hairline);
  text-align: left;
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-standard);
}

.row:hover {
  background: var(--color-surface-hover);
}

.row__title {
  font-size: var(--text-base);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row__meta {
  flex: none;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
