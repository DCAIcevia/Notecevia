import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { useConfigStore } from "./stores/config";
import "./theme/default.css";
import "./theme/base.css";
import "./layout/frame.css";

const pinia = createPinia();
createApp(App).use(pinia).use(router).mount("#app");

// 开发期排查启动用：地址带 ?debug 时在左下角显示耗时。
if (import.meta.env.DEV && new URLSearchParams(window.location.search).has("debug")) {
  const boot = performance.now();
  const mounted = performance.now();
  const badge = document.createElement("div");
  badge.style.cssText =
    "position:fixed;left:8px;bottom:6px;z-index:99999;font:11px/1.4 ui-monospace,monospace;color:#66665f;background:#f5f5f3cc;padding:2px 6px;border-radius:4px;pointer-events:none";
  badge.textContent = `脚本 ${Math.round(mounted)}ms · 挂载完成`;
  document.body.appendChild(badge);

  const timer = window.setInterval(() => {
    if (document.querySelector(".home__list li, .home__state")) {
      badge.textContent = `首屏 ${Math.round(performance.now() - boot)}ms（脚本 ${Math.round(mounted)}ms）`;
      window.clearInterval(timer);
    }
  }, 50);
}

// 配置读不到不挡界面，缺省值照常工作。
void useConfigStore(pinia).load();