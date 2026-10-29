<script setup lang="ts">
// 自绘标题栏：把系统标题栏合进应用。左边应用名，整条可拖动，右边窗口按钮。
// 浏览器里没有窗口对象，所有调用吞掉错误，界面照常可用。
import { onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Restore, Square, X } from "../icons";

const maximized = ref(false);
let stopListening: (() => void) | null = null;

function run(action: () => Promise<unknown>): void {
  void action().catch(() => {
    // 不在 Tauri 窗口里时忽略，例如浏览器预览。
  });
}

async function refresh(): Promise<void> {
  try {
    maximized.value = await getCurrentWindow().isMaximized();
  } catch {
    maximized.value = false;
  }
}

onMounted(async () => {
  await refresh();
  try {
    stopListening = await getCurrentWindow().onResized(() => {
      void refresh();
    });
  } catch {
    stopListening = null;
  }
});

onBeforeUnmount(() => stopListening?.());
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <span class="titlebar__brand" data-tauri-drag-region>Notecevia</span>
    <span class="titlebar__drag" data-tauri-drag-region />
    <div class="titlebar__buttons">
      <button
        class="titlebar__button"
        type="button"
        aria-label="最小化"
        @click="run(() => getCurrentWindow().minimize())"
      >
        <Minus :size="14" :stroke-width="1.6" />
      </button>
      <button
        class="titlebar__button"
        type="button"
        :aria-label="maximized ? '还原' : '最大化'"
        @click="run(() => getCurrentWindow().toggleMaximize())"
      >
        <Restore v-if="maximized" :size="13" :stroke-width="1.6" />
        <Square v-else :size="12" :stroke-width="1.6" />
      </button>
      <button
        class="titlebar__button titlebar__button--close"
        type="button"
        aria-label="关闭"
        @click="run(() => getCurrentWindow().close())"
      >
        <X :size="14" :stroke-width="1.6" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  flex: none;
  align-items: center;
  height: 34px;
  padding-left: var(--space-3);
  user-select: none;
  -webkit-user-select: none;
}

.titlebar__brand {
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  letter-spacing: 0.02em;
}

.titlebar__drag {
  flex: 1;
  align-self: stretch;
}

.titlebar__buttons {
  display: flex;
  align-self: stretch;
}

.titlebar__button {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  border: none;
  background: none;
  color: var(--color-text-muted);
  cursor: default;
}

.titlebar__button:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.titlebar__button--close:hover {
  background: var(--color-danger);
  color: var(--color-text-on-accent);
}
</style>
