<script setup lang="ts">
// 界面验收页：主题变量与基础组件摊开成能看见的东西。
// 真实视图接上后删除本文件与 App.vue 里的引用。

import { ref } from "vue";
import BaseButton from "../components/base/BaseButton.vue";
import BaseCheckbox from "../components/base/BaseCheckbox.vue";
import BaseEmptyState from "../components/base/BaseEmptyState.vue";
import BaseProgress from "../components/base/BaseProgress.vue";
import BaseSearchField from "../components/base/BaseSearchField.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import BaseTextField from "../components/base/BaseTextField.vue";

const surfaces = ["app", "content", "muted", "overlay"] as const;
const shadows = ["hairline", "control", "raised", "overlay", "modal"] as const;
const radii = ["sm", "md", "lg", "xl"] as const;

const query = ref("");
const lastSearch = ref("还没有搜索");
const title = ref("");
const dense = ref(true);
const saved = ref(35);
</script>

<template>
  <div class="preview">
    <section class="preview__section">
      <h1 class="preview__title">默认主题与基础组件</h1>
      <p class="preview__note">一套浅色外观，不跟随系统外观。</p>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">文字</h2>
      <p class="preview__lg">标题字。用于笔记标题、册名。</p>
      <p>正文。列表行、块内文字。</p>
      <p class="preview__muted">次要文字。时间、条数、路径。</p>
      <p class="preview__disabled">禁用文字。</p>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">表面</h2>
      <div class="preview__row">
        <div v-for="surface in surfaces" :key="surface" class="swatch">
          <div class="swatch__plane" :style="{ background: `var(--color-surface-${surface})` }" />
          <code class="swatch__name">{{ surface }}</code>
        </div>
      </div>
    </section>

    <section id="shadow" class="preview__section">
      <h2 class="preview__heading">阴影</h2>
      <div class="preview__row preview__row--soft">
        <div
          v-for="level in shadows"
          :key="level"
          class="tile"
          :style="{ boxShadow: `var(--shadow-${level})` }"
        >
          <code class="tile__name">{{ level }}</code>
        </div>
      </div>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">圆角与同心内缩</h2>
      <div class="preview__row preview__row--soft">
        <div
          v-for="radius in radii"
          :key="radius"
          class="radius"
          :style="{ borderRadius: `var(--radius-${radius})` }"
        >
          <code class="tile__name">{{ radius }}</code>
        </div>
        <div class="nested">
          <div class="nested__inner"><code class="tile__name">12 − 6</code></div>
        </div>
      </div>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">按钮</h2>
      <div class="preview__row">
        <BaseButton variant="primary">新建笔记</BaseButton>
        <BaseButton>取消</BaseButton>
        <BaseButton variant="ghost">更多</BaseButton>
        <BaseButton variant="danger">删除</BaseButton>
        <BaseButton disabled>不可用</BaseButton>
      </div>
      <div class="preview__row">
        <BaseButton size="sm">小号</BaseButton>
        <BaseButton size="sm" variant="primary">小号主要</BaseButton>
      </div>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">输入</h2>
      <div class="preview__row preview__row--baseline">
        <BaseTextField v-model="title" placeholder="笔记标题" />
        <BaseSearchField v-model="query" @search="lastSearch = $event || '清空了'" />
      </div>
      <p class="preview__note">搜索停手 250ms 才发查询：{{ lastSearch }}</p>
      <div class="preview__row preview__row--baseline">
        <BaseTextField :model-value="''" invalid error="标题不能为空。" placeholder="空标题" />
        <BaseTextField :model-value="''" disabled placeholder="不可编辑" />
      </div>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">勾选</h2>
      <div class="preview__row">
        <BaseCheckbox v-model="dense">紧凑列表</BaseCheckbox>
        <BaseCheckbox :model-value="false" disabled>不可用</BaseCheckbox>
      </div>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">进度与等待</h2>
      <div class="preview__stack">
        <BaseProgress :value="saved" label="保存进度" />
        <BaseProgress label="正在重建索引" />
        <p class="preview__note">已保存 {{ saved }}%</p>
        <p class="preview__note"><BaseSpinner /> 正在读取笔记</p>
        <div class="preview__row">
          <BaseButton size="sm" @click="saved = Math.max(0, saved - 20)">退一步</BaseButton>
          <BaseButton size="sm" @click="saved = Math.min(100, saved + 20)">进一步</BaseButton>
        </div>
      </div>
    </section>

    <section class="preview__section">
      <h2 class="preview__heading">空态</h2>
      <BaseEmptyState title="还没有笔记" hint="笔记是内容的容器，先建一条。">
        <template #action>
          <BaseButton variant="primary">新建笔记</BaseButton>
        </template>
      </BaseEmptyState>
    </section>
  </div>
</template>

<style scoped>
.preview {
  height: 100%;
  overflow: auto;
  max-width: 760px;
  padding: var(--space-8);
  background: var(--color-surface-content);
}

.preview__section + .preview__section {
  margin-top: var(--space-8);
  padding-top: var(--space-6);
  border-top: 1px solid var(--color-line);
}

.preview__title {
  font-size: var(--text-xl);
  line-height: var(--leading-tight);
  font-weight: 600;
}

.preview__heading {
  margin-bottom: var(--space-3);
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-muted);
}

.preview__note,
.preview__muted {
  margin-top: var(--space-1);
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.preview__lg {
  font-size: var(--text-lg);
  line-height: var(--leading-tight);
}

.preview__disabled {
  color: var(--color-text-disabled);
}

.preview__row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  align-items: center;
}

.preview__row--baseline {
  align-items: flex-start;
}

.preview__row + .preview__row {
  margin-top: var(--space-3);
}

/* 给阴影留出可见的边距 */
.preview__row--soft {
  padding: var(--space-4);
  background: var(--color-surface-app);
  border-radius: var(--radius-lg);
}

.tile {
  display: flex;
  align-items: flex-end;
  justify-content: center;
  width: 104px;
  height: 56px;
  padding: var(--space-2);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
}

.tile__name {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--color-text-muted);
}

.radius {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 56px;
  height: 56px;
  background: var(--color-surface-content);
  box-shadow: var(--shadow-hairline);
}

.nested {
  display: flex;
  width: 72px;
  height: 72px;
  padding: 6px;
  border-radius: var(--radius-xl);
  background: var(--color-surface-muted);
  box-shadow: var(--shadow-hairline);
}

.nested__inner {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  border-radius: calc(var(--radius-xl) - 6px);
  background: var(--color-surface-content);
}

.preview__stack {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.swatch {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.swatch__plane {
  width: 96px;
  height: 56px;
  border: 1px solid var(--color-line);
  border-radius: var(--radius-md);
}

.swatch__name {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--color-text-muted);
}
</style>