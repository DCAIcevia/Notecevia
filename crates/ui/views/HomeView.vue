<script setup lang="ts">
// 首页：册的封面墙。新建页与新建册都在这里，命名就地完成。
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import AppShell from "../layout/AppShell.vue";
import BaseInlineField from "../components/base/BaseInlineField.vue";
import BaseSearchField from "../components/base/BaseSearchField.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import { Pencil, Trash } from "../icons";
import { api } from "../api/client";
import type { VolumeSummary } from "../api/types";
import { useWorkspaceStore } from "../stores/workspace";

const router = useRouter();
const workspace = useWorkspaceStore();

const volumes = ref<VolumeSummary[]>([]);
const covers = ref(new Map<number, string>());
const loading = ref(true);
const failure = ref<string | null>(null);
const naming = ref<"page" | "volume" | null>(null);
const creating = ref(false);
const query = ref("");
const renamingId = ref<number | null>(null);

/** 0 号册是兜底，不上墙；它有页时在网格末尾占一格「未归档」。 */
const shown = computed(() => volumes.value.filter((volume) => volume.id !== 0));
const filtered = computed(() => {
  const needle = query.value.trim();
  if (!needle) return shown.value;
  return shown.value.filter((volume) => volume.title.includes(needle));
});
const orphans = computed(
  () => volumes.value.find((volume) => volume.id === 0 && volume.pageCount > 0) ?? null,
);

async function load(): Promise<void> {
  loading.value = true;
  failure.value = null;
  try {
    if (!workspace.status) await workspace.open();
    volumes.value = await api.listVolumes();
    await loadCovers();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "读取册失败";
  } finally {
    loading.value = false;
  }
}

/** 封面是缓存：取这本册第一页的缩略图，取不到就留纸色空封面。 */
async function loadCovers(): Promise<void> {
  for (const volume of shown.value) {
    if (covers.value.has(volume.id)) continue;
    try {
      const loaded = await api.loadVolume(volume.id);
      const first = loaded.pages[0];
      if (first === undefined) continue;
      const bytes = await api.readThumbnail(first);
      if (bytes.byteLength === 0) continue;
      const next = new Map(covers.value);
      next.set(volume.id, URL.createObjectURL(new Blob([bytes], { type: "image/png" })));
      covers.value = next;
    } catch {
      // 封面只是缓存，读不到不影响进册。
    }
  }
}

onMounted(load);

function open(volumeId: number): void {
  void router.push({ name: "volume", params: { volumeId } });
}

function beginRename(volume: VolumeSummary): void {
  failure.value = null;
  renamingId.value = volume.id;
}

async function commitRename(volume: VolumeSummary, title: string): Promise<void> {
  renamingId.value = null;
  const next = title.trim();
  if (!next || next === volume.title) return;
  try {
    const loaded = await api.loadVolume(volume.id);
    await api.saveVolume(volume.id, { ...loaded, title: next });
    volumes.value = volumes.value.map((item) =>
      item.id === volume.id ? { ...item, title: next } : item,
    );
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "改名失败";
  }
}

async function removeVolume(volume: VolumeSummary): Promise<void> {
  failure.value = null;
  try {
    await api.deleteVolume(volume.id);
    volumes.value = volumes.value.filter((item) => item.id !== volume.id);
    const url = covers.value.get(volume.id);
    if (url) URL.revokeObjectURL(url);
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "删除失败";
  }
}

async function createPage(title: string): Promise<void> {
  naming.value = null;
  creating.value = true;
  failure.value = null;
  try {
    const created = await api.createPage(title);
    await router.push({
      name: "volume",
      params: { volumeId: created.volume.id, boardId: created.board.id },
    });
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "新建页失败";
  } finally {
    creating.value = false;
  }
}

async function createVolume(title: string): Promise<void> {
  naming.value = null;
  creating.value = true;
  failure.value = null;
  try {
    const created = await api.createVolume(title);
    await router.push({ name: "volume", params: { volumeId: created.id } });
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "新建册失败";
  } finally {
    creating.value = false;
  }
}
</script>

<template>
  <AppShell>
    <header class="wall__bar">
      <h1 class="wall__title">全部册</h1>
      <BaseSearchField class="wall__search" placeholder="找册" @search="query = $event" />
      <span v-if="failure" class="wall__error">{{ failure }}</span>
      <BaseInlineField
        v-if="naming === 'volume'"
        class="wall__naming"
        placeholder="册名"
        label="新建册"
        @submit="createVolume"
        @cancel="naming = null"
      />
      <button
        v-else
        class="wall__action"
        type="button"
        :disabled="creating"
        @click="naming = 'volume'"
      >
        新建册
      </button>
    </header>

    <div class="wall__body">
      <div v-if="loading" class="wall__state"><BaseSpinner /></div>

      <template v-else>
        <p v-if="filtered.length === 0 && shown.length > 0" class="wall__hint">没有命中的册</p>

        <div class="wall__grid">
          <div class="cover cover--draft">
            <div class="cover__frame cover__frame--draft">
              <BaseInlineField
                v-if="naming === 'page'"
                class="cover__naming"
                placeholder="页名"
                label="新建页"
                @submit="createPage"
                @cancel="naming = null"
              />
              <button
                v-else
                class="cover__add"
                type="button"
                :disabled="creating"
                @click="naming = 'page'"
              >
                ＋ 新建页
              </button>
            </div>
            <span class="cover__title">新建页</span>
          </div>

          <div v-for="volume in filtered" :key="volume.id" class="cover">
            <button class="cover__open" type="button" @click="open(volume.id)">
              <span class="cover__frame">
                <img
                  v-if="covers.get(volume.id)"
                  class="cover__image"
                  :src="covers.get(volume.id)"
                  alt=""
                />
              </span>
            </button>
            <div class="cover__foot">
              <BaseInlineField
                v-if="renamingId === volume.id"
                class="cover__rename"
                :initial="volume.title"
                placeholder="册名"
                label="册名"
                @submit="commitRename(volume, $event)"
                @cancel="renamingId = null"
              />
              <span v-else class="cover__title">{{ volume.title }}</span>
              <span class="cover__actions">
                <button
                  class="cover__action"
                  type="button"
                  aria-label="重命名册"
                  title="重命名"
                  @click="beginRename(volume)"
                >
                  <Pencil :size="13" :stroke-width="1.75" />
                </button>
                <button
                  class="cover__action cover__action--danger"
                  type="button"
                  aria-label="删除册"
                  title="删除"
                  @click="removeVolume(volume)"
                >
                  <Trash :size="13" :stroke-width="1.75" />
                </button>
              </span>
            </div>
            <span class="cover__meta">{{ volume.pageCount }} 页</span>
          </div>

          <button v-if="orphans && query.trim() === ''" class="cover" type="button" @click="open(0)">
            <span class="cover__frame cover__frame--plain">
              <span class="cover__mark">{{ orphans.pageCount }} 页</span>
            </span>
            <span class="cover__title">未归档</span>
          </button>
        </div>
      </template>
    </div>
  </AppShell>
</template>

<style scoped>
.wall__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  min-height: 44px;
  padding: var(--space-2) var(--space-4);
  border-bottom: 1px solid var(--color-line);
}

.wall__title {
  flex: 1;
  font-size: var(--text-base);
  font-weight: 600;
}

.wall__search {
  flex: none;
  width: 220px;
}

.wall__hint {
  padding-bottom: var(--space-3);
  color: var(--color-text-muted);
  font-size: var(--text-sm);
}

.wall__error {
  max-width: 40ch;
  overflow: hidden;
  color: var(--color-danger);
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.wall__naming {
  width: 200px;
}

.wall__action {
  padding: 0 var(--space-2);
  height: var(--control-height-md);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  color: var(--color-text);
  font-size: var(--text-sm);
  cursor: pointer;
}

.wall__action:hover:not(:disabled) {
  background: var(--color-surface-hover);
}

.wall__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--space-4);
}

.wall__state {
  display: flex;
  justify-content: center;
  padding: var(--space-8) 0;
}

.wall__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--space-4);
}

.cover {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: 0;
  border: none;
  background: none;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.cover__frame {
  display: block;
  aspect-ratio: 4 / 3;
  overflow: hidden;
  border-radius: var(--radius-lg);
  background: var(--color-surface-muted);
  box-shadow: var(--shadow-hairline);
}

.cover:hover .cover__frame {
  background: var(--color-surface-hover);
}

.cover:hover .cover__image {
  filter: brightness(0.96);
}

.cover__image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover__open {
  display: block;
  padding: 0;
  border: none;
  background: none;
  cursor: pointer;
}

.cover__foot {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  min-height: 24px;
}

.cover__title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cover__rename {
  flex: 1;
}

.cover__actions {
  display: flex;
  flex: none;
  gap: 2px;
  opacity: 0;
}

.cover:hover .cover__actions,
.cover:focus-within .cover__actions {
  opacity: 1;
}

.cover__action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.cover__action:hover {
  background: var(--color-surface-content);
  color: var(--color-text);
}

.cover__action--danger:hover {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}

.cover__meta {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.cover__frame--draft {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--color-line-strong);
  background: none;
  box-shadow: none;
}

.cover__frame--plain {
  display: flex;
  align-items: center;
  justify-content: center;
}

.cover__mark {
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.cover__add {
  padding: var(--space-2);
  border: none;
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.cover__add:hover:not(:disabled) {
  color: var(--color-text);
}

.cover__naming {
  width: 82%;
}

</style>
