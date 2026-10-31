<script setup lang="ts">
// 首页：册的封面墙。新建页与新建册都在这里，命名就地完成。
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import AppShell from "../layout/AppShell.vue";
import BaseInlineField from "../components/base/BaseInlineField.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
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

/** 0 号册是兜底，不进封面墙；它有页时单独一行「未归档」。 */
const shown = computed(() => volumes.value.filter((volume) => volume.id !== 0));
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
      <h1 class="wall__title">册</h1>
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

      <div v-else-if="shown.length === 0 && naming !== 'page'" class="wall__empty">
        <p class="wall__hint">还没有册。</p>
        <button class="wall__action" type="button" @click="naming = 'page'">新建页</button>
      </div>

      <div v-else class="wall__grid">
        <button
          v-for="volume in shown"
          :key="volume.id"
          class="cover"
          type="button"
          @click="open(volume.id)"
        >
          <span class="cover__frame">
            <img v-if="covers.get(volume.id)" class="cover__image" :src="covers.get(volume.id)" alt="" />
          </span>
          <span class="cover__title">{{ volume.title }}</span>
          <span class="cover__meta">{{ volume.pageCount }} 页</span>
        </button>

        <div class="cover cover--draft">
          <BaseInlineField
            v-if="naming === 'page'"
            class="cover__naming"
            placeholder="页名"
            label="新建页"
            @submit="createPage"
            @cancel="naming = null"
          />
          <button v-else class="cover__add" type="button" :disabled="creating" @click="naming = 'page'">
            ＋ 新建页
          </button>
        </div>
      </div>

      <button v-if="orphans" class="wall__orphans" type="button" @click="open(0)">
        未归档 {{ orphans.pageCount }} 页
      </button>
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

.wall__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8) 0;
}

.wall__hint {
  color: var(--color-text-muted);
  font-size: var(--text-sm);
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

.cover__image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover__title {
  overflow: hidden;
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cover__meta {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.cover--draft {
  cursor: default;
}

.cover--draft .cover__frame,
.cover--draft .cover__naming {
  display: flex;
  align-items: center;
  justify-content: center;
}

.cover--draft {
  justify-content: flex-start;
}

.cover__add {
  display: flex;
  align-items: center;
  justify-content: center;
  aspect-ratio: 4 / 3;
  border: 1px dashed var(--color-line-strong);
  border-radius: var(--radius-lg);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.cover__add:hover:not(:disabled) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.cover__naming {
  width: 100%;
}

.wall__orphans {
  margin-top: var(--space-4);
  padding: 0;
  border: none;
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  text-decoration: underline;
  cursor: pointer;
}
</style>
