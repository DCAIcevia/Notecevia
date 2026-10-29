<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import BaseButton from "../components/base/BaseButton.vue";
import BaseEmptyState from "../components/base/BaseEmptyState.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import { api } from "../api/client";
import type { ShelfSummary, VolumeSummary } from "../api/types";

const router = useRouter();
const volumes = ref<VolumeSummary[]>([]);
const shelves = ref<ShelfSummary[]>([]);
const loading = ref(true);
const failure = ref<string | null>(null);

async function load(): Promise<void> {
  loading.value = true;
  failure.value = null;
  try {
    volumes.value = await api.listVolumes();
    shelves.value = await api.listShelves();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "读取册与书架失败";
  } finally {
    loading.value = false;
  }
}

onMounted(load);

async function createVolume(): Promise<void> {
  await api.createVolume("未命名册");
  await load();
}

async function createShelf(): Promise<void> {
  await api.createShelf("未命名书架");
  await load();
}

async function removeVolume(volume: VolumeSummary): Promise<void> {
  failure.value = null;
  try {
    await api.deleteVolume(volume.id);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "删除失败";
  }
}

async function removeShelf(shelf: ShelfSummary): Promise<void> {
  failure.value = null;
  try {
    await api.deleteShelf(shelf.id);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "删除失败";
  }
}
</script>

<template>
  <main class="library">
    <header class="library__bar">
      <button class="library__back" type="button" @click="router.push({ name: 'home' })">← 画板</button>
      <h1 class="library__title">册与书架</h1>
    </header>

    <p v-if="failure" class="library__error">{{ failure }}</p>

    <div v-if="loading" class="library__state"><BaseSpinner /></div>

    <template v-else>
      <section class="library__section">
        <div class="library__head">
          <h2 class="library__heading">册</h2>
          <BaseButton size="sm" @click="createVolume">新建册</BaseButton>
        </div>
        <ul class="library__list">
          <li v-for="volume in volumes" :key="volume.id" class="item">
            <button class="row" type="button" @click="router.push({ name: 'volume', params: { id: volume.id } })">
              <span class="row__title">{{ volume.title }}</span>
              <span class="row__meta">{{ volume.pageCount }} 页</span>
            </button>
            <span class="item__actions">
              <button
                class="item__action item__action--danger"
                type="button"
                :disabled="volume.id === 0"
                @click="removeVolume(volume)"
              >
                删除
              </button>
            </span>
          </li>
        </ul>
      </section>

      <section class="library__section">
        <div class="library__head">
          <h2 class="library__heading">书架</h2>
          <BaseButton size="sm" @click="createShelf">新建书架</BaseButton>
        </div>
        <BaseEmptyState v-if="shelves.length === 0" title="还没有书架" hint="书架用来分门别类地放册。" />
        <ul v-else class="library__list">
          <li v-for="shelf in shelves" :key="shelf.id" class="item">
            <div class="row row--static">
              <span class="row__title">{{ shelf.title }}</span>
              <span class="row__meta">{{ shelf.volumeCount }} 册</span>
            </div>
            <span class="item__actions">
              <button class="item__action item__action--danger" type="button" @click="removeShelf(shelf)">
                删除
              </button>
            </span>
          </li>
        </ul>
      </section>
    </template>
  </main>
</template>

<style scoped>
.library {
  height: 100%;
  overflow: auto;
  padding: var(--space-6) var(--space-8);
  background: var(--color-surface-app);
}

.library__bar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  max-width: 760px;
  margin: 0 auto var(--space-4);
}

.library__back,
.library__title {
  border: none;
  background: none;
  color: inherit;
}

.library__back {
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-sm);
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.library__back:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.library__title {
  font-size: var(--text-xl);
  font-weight: 600;
}

.library__error {
  max-width: 760px;
  margin: 0 auto var(--space-2);
  color: var(--color-danger);
}

.library__state {
  display: flex;
  justify-content: center;
  padding: var(--space-12) 0;
}

.library__section {
  max-width: 760px;
  margin: 0 auto var(--space-8);
}

.library__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-3);
}

.library__heading {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-muted);
}

.library__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.row {
  display: flex;
  flex: 1;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-4);
  min-width: 0;
  padding: var(--space-3);
  border: none;
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-hairline);
  text-align: left;
  cursor: pointer;
}

.row:hover {
  background: var(--color-surface-hover);
}

.row--static {
  cursor: default;
}

.row__title {
  overflow: hidden;
  font-size: var(--text-base);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row__meta {
  flex: none;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.item__actions {
  display: flex;
  flex: none;
  gap: var(--space-1);
  opacity: 0;
}

.item:hover .item__actions,
.item:focus-within .item__actions {
  opacity: 1;
}

.item__action {
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.item__action:disabled {
  color: var(--color-text-disabled);
  cursor: default;
}

.item__action--danger:hover:not(:disabled) {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}
</style>
