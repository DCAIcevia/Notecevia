// 缩略图：把画板画成一张 PNG。缩略图是界面缓存，画不出来就跳过，不影响保存。
//
// 这里重复了一遍块的画法，是有意的：缩略图只需要静态结果，不引 DOM 渲染，
// 也不用把视图缩到全貌再截图，避免用户看到闪一帧。

import type { Body, DocumentDto, Entry, Rgb } from "../api/types";
import { mediaUrl } from "./media";

const PADDING = 24;

function cssColor(value: Rgb | null | undefined, fallback: string): string {
  return value ? `rgb(${value[0]}, ${value[1]}, ${value[2]})` : fallback;
}

function bounds(entries: Entry[]): { x: number; y: number; width: number; height: number } {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const entry of entries) {
    const { geometry } = entry;
    minX = Math.min(minX, geometry.x);
    minY = Math.min(minY, geometry.y);
    maxX = Math.max(maxX, geometry.x + geometry.width);
    maxY = Math.max(maxY, geometry.y + geometry.height);
  }
  return {
    x: minX - PADDING,
    y: minY - PADDING,
    width: Math.max(1, maxX - minX + PADDING * 2),
    height: Math.max(1, maxY - minY + PADDING * 2),
  };
}

function wrap(context: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split("\n")) {
    let line = "";
    for (const character of paragraph) {
      if (context.measureText(line + character).width > maxWidth && line) {
        lines.push(line);
        line = character;
      } else {
        line += character;
      }
    }
    lines.push(line);
  }
  return lines;
}

async function drawImage(
  context: CanvasRenderingContext2D,
  body: Extract<Body, { kind: "image" }>,
  entry: Entry,
): Promise<void> {
  try {
    const url = await mediaUrl(body.media, body.name);
    const image = new Image();
    image.src = url;
    await image.decode();
    const { x, y, width, height } = entry.geometry;
    const fit =
      body.fit === "cover"
        ? Math.max(width / image.width, height / image.height)
        : Math.min(width / image.width, height / image.height);
    const drawWidth = body.fit === "fill" ? width : image.width * fit;
    const drawHeight = body.fit === "fill" ? height : image.height * fit;
    const offsetX = x + (width - drawWidth) / 2;
    const offsetY = y + (height - drawHeight) / 2;
    context.save();
    context.beginPath();
    context.rect(x, y, width, height);
    context.clip();
    context.drawImage(image, offsetX, offsetY, drawWidth, drawHeight);
    context.restore();
  } catch {
    context.fillStyle = "#ececea";
    context.fillRect(entry.geometry.x, entry.geometry.y, entry.geometry.width, entry.geometry.height);
  }
}

function drawBlock(context: CanvasRenderingContext2D, entry: Entry, body: Body | undefined): void {
  const { x, y, width, height } = entry.geometry;
  if (!body) return;

  switch (body.kind) {
    case "text": {
      if (body.background) {
        context.fillStyle = cssColor(body.background, "#ffffff");
        context.fillRect(x, y, width, height);
      }
      const size = body.size ?? 16;
      context.fillStyle = cssColor(body.color, "#1f1f1e");
      context.font = `${size}px ${body.font ?? "system-ui, sans-serif"}`;
      context.textBaseline = "top";
      let cursor = y;
      for (const line of wrap(context, body.text, width)) {
        if (cursor > y + height) break;
        context.fillText(line, x, cursor);
        cursor += size * 1.5;
      }
      break;
    }
    case "table": {
      const cellWidth = width / Math.max(1, body.cols);
      const cellHeight = height / Math.max(1, body.rows);
      context.strokeStyle = "#d0d0cb";
      context.lineWidth = 1;
      context.fillStyle = "#1f1f1e";
      context.font = "13px system-ui, sans-serif";
      context.textBaseline = "top";
      for (let row = 0; row < body.rows; row += 1) {
        for (let column = 0; column < body.cols; column += 1) {
          const cellX = x + column * cellWidth;
          const cellY = y + row * cellHeight;
          context.strokeRect(cellX, cellY, cellWidth, cellHeight);
          const text = body.cells[row * body.cols + column] ?? "";
          if (text) context.fillText(text, cellX + 3, cellY + 3, cellWidth - 6);
        }
      }
      break;
    }
    case "shape": {
      if (body.shape === "line") {
        context.strokeStyle = cssColor(body.stroke, "#d0d0cb");
        context.lineWidth = Math.max(1, body.strokeWidth);
        context.beginPath();
        context.moveTo(x, y + height / 2);
        context.lineTo(x + width, y + height / 2);
        context.stroke();
        break;
      }
      context.beginPath();
      if (body.shape === "ellipse") {
        context.ellipse(x + width / 2, y + height / 2, width / 2, height / 2, 0, 0, Math.PI * 2);
      } else {
        context.rect(x, y, width, height);
      }
      if (body.fill) {
        context.fillStyle = cssColor(body.fill, "#ffffff");
        context.fill();
      }
      if (body.stroke) {
        context.strokeStyle = cssColor(body.stroke, "#d0d0cb");
        context.lineWidth = Math.max(1, body.strokeWidth);
        context.stroke();
      }
      break;
    }
    case "highlight": {
      context.fillStyle = `rgba(${body.color[0]}, ${body.color[1]}, ${body.color[2]}, ${body.opacity})`;
      context.fillRect(x, y, width, height);
      break;
    }
    case "media": {
      context.fillStyle = "#ececea";
      context.fillRect(x, y, width, height);
      context.fillStyle = "#66665f";
      context.font = "13px system-ui, sans-serif";
      context.textBaseline = "middle";
      context.fillText(body.name || "媒体", x + 8, y + height / 2, width - 16);
      break;
    }
    default:
      break;
  }
}

/** 画成 PNG 字节；画布不可用或没有内容时返回 null。 */
export async function renderThumbnail(opened: DocumentDto, targetWidth = 320): Promise<number[] | null> {
  const entries = opened.entries.filter((entry) => entry.visible);
  if (entries.length === 0) return null;

  const box = bounds(entries);
  const scale = Math.min(1, targetWidth / box.width);
  const width = Math.round(box.width * scale);
  const height = Math.round(box.height * scale);

  const canvas = window.document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return null;

  context.fillStyle = "#ffffff";
  context.fillRect(0, 0, width, height);
  context.scale(scale, scale);
  context.translate(-box.x, -box.y);

  const bodies = new Map(opened.bodies.map((body) => [body.id, body]));
  const ordered = entries.slice().sort((left, right) => left.z - right.z || left.id - right.id);
  for (const entry of ordered) {
    context.save();
    const geometry = entry.geometry;
    if (geometry.rotation) {
      context.translate(geometry.x + geometry.width / 2, geometry.y + geometry.height / 2);
      context.rotate(geometry.rotation);
      context.translate(-(geometry.x + geometry.width / 2), -(geometry.y + geometry.height / 2));
    }
    const body = bodies.get(entry.id);
    if (body?.kind === "image") {
      await drawImage(context, body, entry);
    } else {
      drawBlock(context, entry, body);
    }
    context.restore();
  }

  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) return null;
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return Array.from(bytes);
}
