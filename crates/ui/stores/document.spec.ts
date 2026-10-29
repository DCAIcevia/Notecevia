// 文档存储的行为：撤销、脏标记与按策略自动存盘。

import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { DocumentDto } from "../api/types";

const mocks = vi.hoisted(() => ({
  openBoard: vi.fn(),
  saveBoard: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: {
    openBoard: mocks.openBoard,
    saveBoard: mocks.saveBoard,
  },
}));

import { useDocumentStore } from "./document";

function sampleDocument(): DocumentDto {
  return {
    board: { title: "测试画板", tags: [] },
    entries: [
      {
        id: 1,
        kind: "text",
        geometry: { x: 0, y: 0, width: 100, height: 50, rotation: 0 },
        z: 0,
        locked: false,
        visible: true,
      },
      {
        id: 2,
        kind: "text",
        geometry: { x: 200, y: 0, width: 100, height: 50, rotation: 0 },
        z: 0,
        locked: true,
        visible: true,
      },
    ],
    bodies: [
      { kind: "text", id: 1, text: "正文" },
      { kind: "text", id: 2, text: "锁住的块" },
    ],
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  mocks.openBoard.mockReset().mockResolvedValue(sampleDocument());
  mocks.saveBoard.mockReset().mockResolvedValue({ updatedAt: 1_761_800_000 });
  vi.useRealTimers();
});

describe("document store", () => {
  it("一次手势只进一条撤销记录，撤销回到改动前", async () => {
    const store = useDocumentStore();
    await store.open(1);

    store.beginGesture();
    store.moveEntry(1, 40, 20);
    store.moveEntry(1, 60, 30);
    store.endGesture();

    expect(store.entry(1)?.geometry.x).toBe(60);
    expect(store.undoDepth).toBe(1);
    expect(store.dirty).toBe(true);

    store.undo();

    expect(store.entry(1)?.geometry.x).toBe(0);
    expect(store.undoDepth).toBe(0);
    expect(store.redoDepth).toBe(1);
    expect(store.dirty).toBe(false);
  });

  it("手势没有改动时不留撤销记录", async () => {
    const store = useDocumentStore();
    await store.open(1);

    store.beginGesture();
    store.endGesture();

    expect(store.undoDepth).toBe(0);
    expect(store.dirty).toBe(false);
  });

  it("重做把撤销掉的那一步放回去", async () => {
    const store = useDocumentStore();
    await store.open(1);

    store.beginGesture();
    store.moveEntry(1, 50, 0);
    store.endGesture();
    store.undo();
    store.redo();

    expect(store.entry(1)?.geometry.x).toBe(50);
    expect(store.redoDepth).toBe(0);
  });

  it("新建块带默认正文，删除时正文一起走", async () => {
    const store = useDocumentStore();
    await store.open(1);

    store.beginGesture();
    const id = store.addEntry("table", { x: 0, y: 0, width: 300, height: 160, rotation: 0 });
    store.endGesture();

    const created = store.document?.bodies.find((body) => body.id === id);
    expect(created?.kind).toBe("table");
    expect(store.undoDepth).toBe(1);

    store.beginGesture();
    store.removeEntry(id);
    store.endGesture();

    expect(store.entry(id)).toBeUndefined();
    expect(store.document?.bodies.some((body) => body.id === id)).toBe(false);
  });

  it("文本写回只在真的变了时动文档", async () => {
    const store = useDocumentStore();
    await store.open(1);

    store.beginGesture();
    store.setText(1, "改了正文");
    store.setText(1, "改了正文");
    store.endGesture();

    expect(store.undoDepth).toBe(1);
    const body = store.document?.bodies.find((item) => item.id === 1);
    expect(body?.kind === "text" ? body.text : "").toBe("改了正文");

    store.undo();
    const restored = store.document?.bodies.find((item) => item.id === 1);
    expect(restored?.kind === "text" ? restored.text : "").toBe("正文");
  });

  it("改动后按策略的间隔存盘", async () => {
    vi.useFakeTimers();
    const store = useDocumentStore();
    await store.open(1);
    store.usePolicy(2000);

    store.beginGesture();
    store.moveEntry(1, 10, 0);
    store.endGesture();

    await vi.advanceTimersByTimeAsync(1999);
    expect(mocks.saveBoard).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.saveBoard).toHaveBeenCalledTimes(1);
    expect(store.dirty).toBe(false);
    expect(store.status).toBe("saved");
  });

  it("存盘失败时保留未保存并留着错误", async () => {
    vi.useFakeTimers();
    mocks.saveBoard.mockRejectedValue({ kind: "io", message: "写盘失败" });
    const store = useDocumentStore();
    await store.open(1);
    store.usePolicy(2000);

    store.beginGesture();
    store.moveEntry(1, 10, 0);
    store.endGesture();
    await vi.advanceTimersByTimeAsync(2000);

    expect(store.status).toBe("failed");
    expect(store.saveError?.message).toBe("写盘失败");
    expect(store.dirty).toBe(true);
  });
});