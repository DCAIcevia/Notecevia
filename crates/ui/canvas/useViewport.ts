// 画布视口：缩放与平移只改变换，不触发重排。

import { computed, ref } from "vue";

const MIN_SCALE = 0.1;
const MAX_SCALE = 4;

export interface Box {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export function useViewport() {
  const x = ref(0);
  const y = ref(0);
  const scale = ref(1);

  const transform = computed(
    () => `translate3d(${x.value}px, ${y.value}px, 0) scale(${scale.value})`,
  );

  /** 以视口内的某一点为中心缩放，该点下方的世界坐标保持不动。 */
  function zoomAt(clientX: number, clientY: number, factor: number): void {
    const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale.value * factor));
    if (next === scale.value) return;
    const ratio = next / scale.value;
    x.value = clientX - (clientX - x.value) * ratio;
    y.value = clientY - (clientY - y.value) * ratio;
    scale.value = next;
  }

  function panBy(dx: number, dy: number): void {
    x.value += dx;
    y.value += dy;
  }

  /** 把整块内容放进视口，留出 padding。内容比视口小时保持 1:1，不放大。 */
  function fitContent(box: Box, width: number, height: number, padding = 64): void {
    const boxWidth = Math.max(1, box.maxX - box.minX);
    const boxHeight = Math.max(1, box.maxY - box.minY);
    const scaleFit = Math.min(
      (width - padding * 2) / boxWidth,
      (height - padding * 2) / boxHeight,
      1,
    );
    scale.value = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scaleFit));
    x.value = (width - boxWidth * scale.value) / 2 - box.minX * scale.value;
    y.value = (height - boxHeight * scale.value) / 2 - box.minY * scale.value;
  }

  return { x, y, scale, transform, zoomAt, panBy, fitContent };
}
