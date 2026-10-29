import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { useConfigStore } from "./stores/config";
import "./theme/default.css";
import "./theme/base.css";

const pinia = createPinia();
createApp(App).use(pinia).use(router).mount("#app");

// 配置读不到不挡界面，缺省值照常工作。
void useConfigStore(pinia).load();
