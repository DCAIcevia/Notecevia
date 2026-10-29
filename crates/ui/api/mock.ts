// 开发期的假数据：没有工作区也能看界面。只在 dev 且地址带 ?mock 时启用。

import type { BoardSummary, DocumentDto } from "./types";

const boards: BoardSummary[] = [
  { id: 1, title: "读书笔记", tags: ["阅读"], updatedAt: 1_761_800_000, entryCount: 12 },
  { id: 2, title: "界面样张", tags: ["设计"], updatedAt: 1_761_790_000, entryCount: 8 },
  { id: 3, title: "随手记", tags: [], updatedAt: 1_761_700_000, entryCount: 3 },
];

const sampleSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200" viewBox="0 0 320 200">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
<stop offset="0" stop-color="#dfe7f5"/><stop offset="1" stop-color="#c8d6ec"/>
</linearGradient></defs>
<rect width="320" height="200" fill="url(#g)"/>
<circle cx="160" cy="100" r="44" fill="#ffffff" fill-opacity="0.7"/>
</svg>`;

function sampleDocument(): DocumentDto {
  return {
    board: { title: "界面样张", tags: ["设计", "样张"] },
    entries: [
      { id: 1, kind: "text", geometry: { x: 0, y: 0, width: 560, height: 56, rotation: 0 }, z: 3, locked: false, visible: true },
      { id: 2, kind: "text", geometry: { x: 0, y: 72, width: 420, height: 132, rotation: 0 }, z: 4, locked: false, visible: true },
      { id: 3, kind: "shape", geometry: { x: 460, y: 74, width: 160, height: 160, rotation: 0 }, z: 1, locked: false, visible: true },
      { id: 4, kind: "image", geometry: { x: 0, y: 228, width: 320, height: 200, rotation: 0 }, z: 2, locked: false, visible: true },
      { id: 5, kind: "table", geometry: { x: 356, y: 228, width: 300, height: 150, rotation: 0 }, z: 2, locked: false, visible: true },
      { id: 6, kind: "highlight", geometry: { x: 0, y: 452, width: 320, height: 108, rotation: 0 }, z: 0, locked: false, visible: true },
      { id: 7, kind: "video", geometry: { x: 356, y: 400, width: 220, height: 132, rotation: 0 }, z: 2, locked: false, visible: true },
      { id: 8, kind: "shape", geometry: { x: 0, y: 588, width: 460, height: 2, rotation: 0 }, z: 1, locked: false, visible: true },
    ],
    bodies: [
      { kind: "text", id: 1, text: "界面样张", size: 28, color: [31, 31, 30] },
      { kind: "text", id: 2, text: "正文用来检查行高、换行与缩放。拖动空白处平移，按住 Ctrl 滚轮缩放。", size: 15, color: null, background: null },
      { kind: "shape", id: 3, shape: "ellipse", fill: [233, 240, 252], stroke: [52, 102, 201], strokeWidth: 2 },
      { kind: "image", id: 4, media: "mock-image", name: "样例.svg", fit: "cover" },
      { kind: "table", id: 5, rows: 3, cols: 3, cells: ["块", "种类", "层级", "文本", "私有", "0", "图片", "共享", "2"] },
      { kind: "highlight", id: 6, color: [255, 224, 102], opacity: 0.35 },
      { kind: "media", id: 7, media: "mock-video", name: "样例.mp4", bytes: 1024 },
      { kind: "shape", id: 8, shape: "line", fill: null, stroke: [208, 208, 203], strokeWidth: 2 },
    ],
  };
}

const encoder = new TextEncoder();

export async function mockInvoke(command: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case "list_volumes":
      return [
        { id: 1, title: "笔记本", pageCount: 2, updatedAt: 1_761_800_000 },
        { id: 0, title: "Empty book", pageCount: 1, updatedAt: 1_761_700_000 },
      ];
    case "load_volume":
      return { title: "笔记本", pages: [1, 2] };
    case "create_volume":
      return { id: 2, title: String(args?.title ?? "未命名册"), pageCount: 0, updatedAt: 1_761_800_000 };
    case "delete_volume":
    case "assign_page":
    case "unassign_page":
    case "move_page_in_volume":
    case "save_shelf":
    case "restore_trash":
    case "purge_trash_item":
      return null;
    case "list_shelves":
      return [{ id: 1, title: "工作", volumeCount: 1, updatedAt: 1_761_800_000 }];
    case "load_shelf":
      return { title: "工作", volumes: [1] };
    case "create_shelf":
      return { id: 2, title: String(args?.title ?? "未命名书架"), volumeCount: 0, updatedAt: 1_761_800_000 };
    case "delete_shelf":
      return null;
    case "list_trash":
      return [
        { folder: "1761700000-board-7", kind: "board", id: 7, title: "旧草稿", deletedAt: 1_761_700_000, bytes: 2048, daysLeft: 29 },
      ];
    case "empty_trash":
      return 1;
    case "app_config":
      return {
        path: "mock/config.json",
        config: { autosave: { intervalMs: 2000, thumbnailIntervalMs: 5000 } },
      };
    case "save_app_config":
      return null;
    case "app_info":
      return { version: "0.2.0-alpha.1", defaultWorkspace: "mock" };
    case "open_workspace":
      return { root: "mock", purgedOnOpen: 0 };
    case "list_boards":
      return boards;
    case "open_board":
      return sampleDocument();
    case "create_board":
      return { id: 99, title: String(args?.title ?? "未命名笔记"), tags: [], updatedAt: 1_761_800_000, entryCount: 0 };
    case "read_thumbnail":
      return new ArrayBuffer(0);
    case "save_thumbnail":
      return null;
    case "rebuild_index":
      return boards.length;
    case "export_container":
      return "mock/exports/sample.notcevia";
    case "export_html":
      return "mock/exports/sample.html";
    case "read_media": {
      const bytes = encoder.encode(sampleSvg);
      return bytes.buffer;
    }
    default:
      throw { kind: "internal", message: `未模拟的命令：${command}` };
  }
}