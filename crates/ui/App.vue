<script setup lang="ts">
import UiPreview from "./preview/UiPreview.vue";
</script>

<template>
  <!-- 应用外壳：真实视图接进来后，把 UiPreview 换成视图容器。 -->
  <main class="app-view">
    <UiPreview />
  </main>
</template>

<style scoped>
.app-view {
  height: 100%;
}
</style>
