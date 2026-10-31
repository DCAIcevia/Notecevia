import { createRouter, createWebHashHistory } from "vue-router";
import HomeView from "./views/HomeView.vue";
import BoardView from "./views/BoardView.vue";
import TrashView from "./views/TrashView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView },
    { path: "/volume/:volumeId/:boardId?", name: "volume", component: BoardView, props: true },
    { path: "/trash", name: "trash", component: TrashView },
  ],
});
