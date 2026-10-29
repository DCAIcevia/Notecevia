<script setup lang="ts">
// 自绘标题栏的左侧内容：应用名 + 可拖动区。
// 窗口按钮由插件画在右侧（带系统 Snap Layout 命中区），这里只让出净空。
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
</script>

<template>
  <header v-if="shell.mode !== 'native'" class="titlebar" data-tauri-drag-region>
    <img class="titlebar__mark" src="/icon.svg" alt="" width="18" height="18" />
    <span class="titlebar__brand" data-tauri-drag-region>Notecevia</span>
    <span class="titlebar__drag" data-tauri-drag-region />
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  flex: none;
  align-items: center;
  height: var(--titlebar-height);
  padding-left: max(var(--space-3), var(--tauri-plugin-decoration-left-clearance, 0px));
  padding-right: var(--tauri-plugin-decoration-right-clearance, 0px);
  user-select: none;
  -webkit-user-select: none;
}

.titlebar__mark {
  flex: none;
  width: 18px;
  height: 18px;
  margin-right: var(--space-2);
}

.titlebar__brand {
  font-size: var(--text-base);
  font-weight: 500;
  color: var(--color-text);
  letter-spacing: 0.01em;
  pointer-events: auto;
}

.titlebar__drag {
  flex: 1;
  align-self: stretch;
  pointer-events: auto;
}
</style>