// 媒体按内容哈希取一次，前端复用同一份 ObjectURL。

import { api } from "../api/client";

const loaded = new Map<string, string>();
const pending = new Map<string, Promise<string>>();

function mimeOf(name: string): string {
  const extension = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  switch (extension) {
    case "png":
      return "image/png";
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "gif":
      return "image/gif";
    case "webp":
      return "image/webp";
    case "avif":
      return "image/avif";
    case "svg":
      return "image/svg+xml";
    case "mp4":
      return "video/mp4";
    case "webm":
      return "video/webm";
    case "mp3":
      return "audio/mpeg";
    case "wav":
      return "audio/wav";
    case "ogg":
      return "audio/ogg";
    default:
      return "application/octet-stream";
  }
}

export function mediaUrl(hash: string, name: string): Promise<string> {
  const cached = loaded.get(hash);
  if (cached) return Promise.resolve(cached);

  const running = pending.get(hash);
  if (running) return running;

  const request = api
    .readMedia(hash)
    .then((bytes) => {
      const url = URL.createObjectURL(new Blob([bytes], { type: mimeOf(name) }));
      loaded.set(hash, url);
      return url;
    })
    .finally(() => pending.delete(hash));

  pending.set(hash, request);
  return request;
}
