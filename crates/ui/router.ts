import { createRouter, createWebHashHistory } from "vue-router";
import HomeView from "./views/HomeView.vue";
import BoardView from "./views/BoardView.vue";
import LibraryView from "./views/LibraryView.vue";
import ShelfView from "./views/ShelfView.vue";
import TrashView from "./views/TrashView.vue";
import VolumeView from "./views/VolumeView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView },
    { path: "/board/:id", name: "board", component: BoardView, props: true },
    { path: "/library", name: "library", component: LibraryView },
    { path: "/volume/:id", name: "volume", component: VolumeView, props: true },
    { path: "/shelf/:id", name: "shelf", component: ShelfView, props: true },
    { path: "/trash", name: "trash", component: TrashView },
  ],
});