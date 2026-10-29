// 图标包没有逐图标的类型声明，按模块取时补一份。
declare module "@lucide/vue/dist/esm/icons/*.mjs" {
  import type { Component } from "vue";
  const icon: Component;
  export default icon;
}
