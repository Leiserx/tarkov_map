<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import Panzoom, { type PanzoomObject } from '@panzoom/panzoom';
import { worldPointToSvg, worldToSvgPosition } from '@/composables/useMapTransform';
import mapLabelsJson from '@/data/map-labels.json';
import mapPointsJson from '@/data/map-points.json';
import type {
  MapLabelItem,
  MapPointsData,
  ScreenshotCoord,
  SvgPosition,
  TarkovMapConfig,
  TeamMemberInfo,
} from '@/types';

const props = defineProps<{
  mapConfig: TarkovMapConfig;
  latestCoord: ScreenshotCoord | null;
  autoCenter: boolean;
  showLabels: boolean;
  showPoints: boolean;
  playerName: string;
  teammates: TeamMemberInfo[];
}>();

const viewportRef = ref<HTMLElement | null>(null);
const stageRef = ref<HTMLElement | null>(null);
const svgLayerRef = ref<HTMLElement | null>(null);

const svgContent = ref('');
const svgWidth = ref(1000);
const svgHeight = ref(1000);
const isLoading = ref(false);
const loadError = ref('');
const currentScale = ref(1);
const pulseKey = ref(0);
const activeLayerId = ref(props.mapConfig.defaultLayer);

const MIN_SCALE = 0.08;
const MAX_SCALE = 16;

let pz: PanzoomObject | null = null;

const playerSvgPos = computed<SvgPosition | null>(() => {
  const { mapConfig } = props;
  if (!props.latestCoord || svgWidth.value <= 0 || svgHeight.value <= 0) {
    return null;
  }
  const position = worldToSvgPosition(
    props.latestCoord,
    mapConfig,
    svgWidth.value,
    svgHeight.value
  );
  if ('affine' in mapConfig && (
    position.svgX < 0 || position.svgX > svgWidth.value ||
    position.svgY < 0 || position.svgY > svgHeight.value
  )) {
    return null;
  }
  return position;
});

const markerScale = computed(() => {
  // .map-stage 宽高与 svgWidth/svgHeight 1:1 对应，因此 1/currentScale 即可让所有地图保持一致的屏幕像素大小
  return Math.min(2.2, Math.max(0.2, 1 / Math.max(0.15, currentScale.value)));
});

/**
 * 将玩家自定义名称置于朝向扇形的反方向（+180°），并根据左右偏移自动对齐文本，避免遮挡红点与视野扇形
 */
const playerNameLayout = computed(() => {
  if (!playerSvgPos.value || !props.playerName.trim()) return null;
  const oppRad = ((playerSvgPos.value.headingDeg + 180) * Math.PI) / 180;
  const x = Math.sin(oppRad) * 18;
  const y = -Math.cos(oppRad) * 18;
  const textAnchor = x > 5 ? 'start' : x < -5 ? 'end' : 'middle';
  return { x, y, textAnchor };
});

const renderedLabels = computed(() => {
  const { mapConfig } = props;
  if (!props.showLabels || svgWidth.value <= 0 || svgHeight.value <= 0) {
    return [];
  }
  const rawLabels =
    (mapLabelsJson as Record<string, MapLabelItem[]>)[mapConfig.id] ?? [];

  return rawLabels.map((item) => {
    const { svgX, svgY } = worldPointToSvg(
      item.position[0],
      item.position[1],
      mapConfig,
      svgWidth.value,
      svgHeight.value
    );
    return {
      text: item.text,
      svgX,
      svgY,
      rotation: item.rotation ?? 0,
      isActiveLayer: item.layerId === activeLayerId.value,
    };
  });
});

const renderedPoints = computed(() => {
  const { mapConfig } = props;
  if (!props.showPoints || svgWidth.value <= 0 || svgHeight.value <= 0) {
    return { extracts: [], transits: [] };
  }
  const data = (mapPointsJson as Record<string, MapPointsData>)[mapConfig.id];
  if (!data) return { extracts: [], transits: [] };

  const factionColor: Record<string, string> = {
    pmc: '#22C55E',
    scav: '#F59E0B',
    shared: '#38BDF8',
  };

  const extracts = (data.extracts ?? []).map((item) => {
    const { svgX, svgY } = worldPointToSvg(
      item.position[0],
      item.position[1],
      mapConfig,
      svgWidth.value,
      svgHeight.value
    );
    return {
      name: item.name,
      faction: item.faction,
      color: factionColor[item.faction] ?? factionColor.shared,
      svgX,
      svgY,
      isActiveLayer: item.layerId === activeLayerId.value,
    };
  });

  const transits = (data.transits ?? []).map((item) => {
    const { svgX, svgY } = worldPointToSvg(
      item.position[0],
      item.position[1],
      mapConfig,
      svgWidth.value,
      svgHeight.value
    );
    return {
      description: item.description,
      svgX,
      svgY,
      isActiveLayer: item.layerId === activeLayerId.value,
    };
  });

  return { extracts, transits };
});

const renderedTeammates = computed(() => {
  const { mapConfig } = props;
  if (svgWidth.value <= 0 || svgHeight.value <= 0) return [];
  return props.teammates
    .filter(
      (m) => m.mapId === mapConfig.id && m.x !== null && m.z !== null
    )
    .map((m) => {
      const { svgX, svgY } = worldPointToSvg(
        m.x as number,
        m.z as number,
        mapConfig,
        svgWidth.value,
        svgHeight.value
      );
      return {
        peerId: m.peerId,
        playerName: m.playerName,
        svgX,
        svgY,
      };
    });
});

function extractSvgDimensions(rawSvg: string): { w: number; h: number } {
  const parser = new DOMParser();
  const doc = parser.parseFromString(rawSvg, 'image/svg+xml');
  const svgEl = doc.querySelector('svg');
  if (!svgEl) return { w: 1000, h: 1000 };

  const viewBox = svgEl.getAttribute('viewBox');
  if (viewBox) {
    const parts = viewBox.trim().split(/[\s,]+/).map(Number);
    if (parts.length === 4 && parts[2] > 0 && parts[3] > 0) {
      return { w: parts[2], h: parts[3] };
    }
  }

  const w = parseFloat(svgEl.getAttribute('width') || '1000') || 1000;
  const h = parseFloat(svgEl.getAttribute('height') || '1000') || 1000;
  return { w, h };
}

function applyLayerVisibility() {
  const root = svgLayerRef.value;
  if (!root) return;

  const defaultId = props.mapConfig.defaultLayer;
  const activeId = activeLayerId.value;
  const isDefaultActive = activeId === defaultId;

  for (const layer of props.mapConfig.layers) {
    const el = root.querySelector<SVGGElement>(`#${CSS.escape(layer.id)}`);
    if (!el) continue;

    if (layer.id === activeId) {
      el.style.display = 'inline';
      el.style.opacity = '1';
    } else if (layer.id === defaultId && !isDefaultActive) {
      // 查看非地面层时，保留半透明地面层作为空间参照
      el.style.display = 'inline';
      el.style.opacity = '0.32';
    } else {
      el.style.display = 'none';
      el.style.opacity = '1';
    }
  }
}

function selectLayer(layerId: string) {
  activeLayerId.value = layerId;
  applyLayerVisibility();
}

let loadSeq = 0;
const svgCache = new Map<string, string>();

async function fetchSvgWithRetry(svgPath: string, retries = 3): Promise<string> {
  const cached = svgCache.get(svgPath);
  if (cached) return cached;

  let lastErr: unknown;
  for (let i = 0; i < retries; i++) {
    try {
      const resp = await fetch(svgPath);
      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status}`);
      }
      const text = await resp.text();
      if (!text.includes('<svg')) {
        throw new Error('无效的 SVG 响应内容');
      }
      svgCache.set(svgPath, text);
      return text;
    } catch (err) {
      lastErr = err;
      if (i < retries - 1) {
        await new Promise((r) => setTimeout(r, 250));
      }
    }
  }
  throw lastErr;
}

async function loadMapSvg(svgPath: string) {
  const seq = ++loadSeq;
  isLoading.value = true;
  loadError.value = '';
  try {
    const text = await fetchSvgWithRetry(svgPath);
    if (seq !== loadSeq) return;

    const { w, h } = extractSvgDimensions(text);
    svgWidth.value = w;
    svgHeight.value = h;
    svgContent.value = text;
    activeLayerId.value = props.mapConfig.defaultLayer;

    await nextTick();
    if (seq !== loadSeq) return;
    applyLayerVisibility();
    resetView();
  } catch (err) {
    if (seq !== loadSeq) return;
    loadError.value = `无法加载地图文件 (${svgPath}): ${String(err)}`;
    svgContent.value = '';
  } finally {
    if (seq === loadSeq) {
      isLoading.value = false;
    }
  }
}

function handlePanzoomChange(e: Event) {
  const detail = (e as CustomEvent<{ scale: number }>).detail;
  if (detail && typeof detail.scale === 'number') {
    currentScale.value = detail.scale;
  }
}

function zoomAtClientPoint(nextScale: number, clientX: number, clientY: number) {
  if (!pz || !viewportRef.value) return;
  const clampedScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, nextScale));
  const oldScale = pz.getScale();
  if (Math.abs(clampedScale - oldScale) < 1e-6) return;

  const rect = viewportRef.value.getBoundingClientRect();
  const mouseX = clientX - rect.left;
  const mouseY = clientY - rect.top;
  const { x: oldPanX, y: oldPanY } = pz.getPan();

  const stageX = mouseX / oldScale - oldPanX;
  const stageY = mouseY / oldScale - oldPanY;

  const nextPanX = mouseX / clampedScale - stageX;
  const nextPanY = mouseY / clampedScale - stageY;

  pz.zoom(clampedScale, { animate: false });
  pz.pan(nextPanX, nextPanY, { animate: false, force: true });
  currentScale.value = clampedScale;
}

let lastLayerWheelTime = 0;

function handleWheel(e: WheelEvent) {
  if (e.ctrlKey) {
    const layers = props.mapConfig.layers;
    if (layers.length <= 1) return;

    const now = performance.now();
    if (now - lastLayerWheelTime < 120) return;
    lastLayerWheelTime = now;

    const currentIdx = layers.findIndex((l) => l.id === activeLayerId.value);
    const baseIdx = currentIdx >= 0 ? currentIdx : 0;
    const step = e.deltaY < 0 ? 1 : -1;
    const nextIdx = (baseIdx + step + layers.length) % layers.length;
    selectLayer(layers[nextIdx].id);
    return;
  }

  if (!pz) return;
  const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
  zoomAtClientPoint(pz.getScale() * factor, e.clientX, e.clientY);
}

function zoomIn() {
  if (!pz || !viewportRef.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  zoomAtClientPoint(pz.getScale() * 1.25, rect.left + rect.width / 2, rect.top + rect.height / 2);
}

function zoomOut() {
  if (!pz || !viewportRef.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  zoomAtClientPoint(pz.getScale() / 1.25, rect.left + rect.width / 2, rect.top + rect.height / 2);
}

function resetView() {
  if (!pz || !viewportRef.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;

  const fitScale = Math.min(rect.width / svgWidth.value, rect.height / svgHeight.value) * 0.92;
  const clampedScale = Math.max(0.1, Math.min(4, fitScale));

  const targetX = (rect.width - svgWidth.value * clampedScale) / 2 / clampedScale;
  const targetY = (rect.height - svgHeight.value * clampedScale) / 2 / clampedScale;

  pz.zoom(clampedScale, { animate: false });
  pz.pan(targetX, targetY, { animate: false, force: true });
  currentScale.value = clampedScale;
}

function centerPlayer() {
  if (!pz || !viewportRef.value || !playerSvgPos.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  const scale = currentScale.value || 1;
  const { svgX, svgY } = playerSvgPos.value;

  const panX = rect.width / (2 * scale) - svgX;
  const panY = rect.height / (2 * scale) - svgY;
  pz.pan(panX, panY, { animate: true, force: true });
}

watch(
  () => props.mapConfig.svgPath,
  (newPath) => {
    loadMapSvg(newPath);
  },
  { immediate: true }
);

watch(
  () => props.latestCoord,
  (coord) => {
    if (!coord) return;
    pulseKey.value += 1;
    if (props.autoCenter) {
      nextTick(() => centerPlayer());
    }
  }
);

onMounted(() => {
  if (stageRef.value) {
    pz = Panzoom(stageRef.value, {
      maxScale: MAX_SCALE,
      minScale: MIN_SCALE,
      step: 0.25,
      origin: '0 0',
    });
    stageRef.value.addEventListener('panzoomchange', handlePanzoomChange);
  }
});

onUnmounted(() => {
  stageRef.value?.removeEventListener('panzoomchange', handlePanzoomChange);
  pz?.destroy();
  pz = null;
});

defineExpose({
  zoomIn,
  zoomOut,
  resetView,
  centerPlayer,
});
</script>

<template>
  <div ref="viewportRef" class="canvas-viewport" @wheel.prevent="handleWheel">
    <div v-if="isLoading" class="canvas-overlay-msg">正在加载地图 SVG...</div>
    <div v-else-if="loadError" class="canvas-overlay-msg is-error">{{ loadError }}</div>

    <div
      ref="stageRef"
      class="map-stage"
      :style="{ width: `${svgWidth}px`, height: `${svgHeight}px` }"
    >
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div ref="svgLayerRef" class="svg-layer" v-html="svgContent" />

      <svg
        v-if="
          playerSvgPos ||
          renderedLabels.length > 0 ||
          renderedPoints.extracts.length > 0 ||
          renderedPoints.transits.length > 0 ||
          renderedTeammates.length > 0
        "
        class="marker-overlay"
        :viewBox="`0 0 ${svgWidth} ${svgHeight}`"
      >
        <g v-if="showLabels && renderedLabels.length > 0" class="map-labels-layer">
          <g
            v-for="(label, idx) in renderedLabels"
            :key="`${mapConfig.id}-lbl-${idx}`"
            :transform="`translate(${label.svgX}, ${label.svgY}) scale(${markerScale})`"
            :opacity="label.isActiveLayer ? 1 : 0.22"
          >
            <text
              x="0"
              y="0"
              :transform="label.rotation ? `rotate(${label.rotation})` : undefined"
              class="map-area-label"
            >
              {{ label.text }}
            </text>
          </g>
        </g>

        <g v-if="showPoints" class="map-points-layer">
          <!-- 撤离点标记 (PMC / Scav / 通用) -->
          <g
            v-for="(ext, idx) in renderedPoints.extracts"
            :key="`${mapConfig.id}-ext-${idx}`"
            :transform="`translate(${ext.svgX}, ${ext.svgY}) scale(${markerScale})`"
            :opacity="ext.isActiveLayer ? 1 : 0.6"
          >
            <circle cx="0" cy="0" r="5" :fill="ext.color" stroke="#0F1115" stroke-width="1.6" />
            <text x="9" y="0" class="point-label" :fill="ext.color">
              {{ ext.name }}
            </text>
          </g>

          <!-- 转移点标记 -->
          <g
            v-for="(tr, idx) in renderedPoints.transits"
            :key="`${mapConfig.id}-tr-${idx}`"
            :transform="`translate(${tr.svgX}, ${tr.svgY}) scale(${markerScale})`"
            :opacity="tr.isActiveLayer ? 1 : 0.6"
          >
            <polygon
              points="0,-6 6,0 0,6 -6,0"
              fill="#C084FC"
              stroke="#0F1115"
              stroke-width="1.5"
            />
            <text x="9" y="0" class="point-label" fill="#E9D5FF">
              {{ tr.description }}
            </text>
          </g>
        </g>

        <!-- 队友位置标记（仅位置圆点与名称，不渲染朝向） -->
        <g v-if="renderedTeammates.length > 0" class="teammates-layer">
          <g
            v-for="mate in renderedTeammates"
            :key="mate.peerId"
            :transform="`translate(${mate.svgX}, ${mate.svgY}) scale(${markerScale})`"
          >
            <circle cx="0" cy="0" r="6" fill="#38BDF8" stroke="#FFFFFF" stroke-width="2" />
            <text x="0" y="-15" text-anchor="middle" class="teammate-name-tag">
              {{ mate.playerName }}
            </text>
          </g>
        </g>

        <g
          v-if="playerSvgPos"
          :transform="`translate(${playerSvgPos.svgX}, ${playerSvgPos.svgY}) scale(${markerScale})`"
        >
          <!-- 脉冲光环 -->
          <circle :key="pulseKey" class="player-pulse" cx="0" cy="0" r="10" />

          <!-- 朝向扇形视野锥 -->
          <g :transform="`rotate(${playerSvgPos.headingDeg})`">
            <path
              d="M 0 0 L -22 -48 A 52 52 0 0 1 22 -48 Z"
              fill="rgba(255, 59, 48, 0.30)"
              stroke="rgba(255, 59, 48, 0.75)"
              stroke-width="1.2"
            />
            <polygon points="0,-16 -5,-5 5,-5" fill="#FF3B30" stroke="#FFFFFF" stroke-width="1" />
          </g>

          <!-- 玩家核心红点 -->
          <circle cx="0" cy="0" r="6.5" fill="#FF3B30" stroke="#FFFFFF" stroke-width="2" />

          <!-- 玩家自定义名称（位于朝向反方向，不遮挡视野扇形） -->
          <text
            v-if="playerNameLayout"
            :x="playerNameLayout.x"
            :y="playerNameLayout.y"
            :text-anchor="playerNameLayout.textAnchor"
            class="player-name-tag"
          >
            {{ playerName }}
          </text>
        </g>
      </svg>
    </div>

    <!-- 多楼层地图的图层切换器（默认选中地面层） -->
    <div
      v-if="mapConfig.layers.length > 1"
      class="layer-switcher"
      title="按住 Ctrl + 滚轮可快速切换楼层图层"
      @wheel.stop
    >
      <button
        v-for="layer in mapConfig.layers"
        :key="layer.id"
        type="button"
        class="layer-btn"
        :class="{ 'is-active': activeLayerId === layer.id }"
        @click="selectLayer(layer.id)"
      >
        {{ layer.label }}
      </button>
    </div>

    <!-- 撤离/转移图例 -->
    <div v-if="showPoints" class="points-legend">
      <span class="legend-item"><i class="dot dot-pmc" />PMC 撤离</span>
      <span class="legend-item"><i class="dot dot-scav" />Scav 撤离</span>
      <span class="legend-item"><i class="dot dot-shared" />通用撤离</span>
      <span class="legend-item"><i class="diamond" />转移点</span>
    </div>
  </div>
</template>

<style scoped>
.canvas-viewport {
  position: relative;
  flex: 1;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: var(--tarkov-bg-canvas);
  cursor: grab;
}

.canvas-viewport:active {
  cursor: grabbing;
}

.map-stage {
  position: relative;
  transform-origin: 0 0;
}

.svg-layer {
  width: 100%;
  height: 100%;
}

.svg-layer :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}

.marker-overlay {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  overflow: visible;
}

.map-area-label {
  font-size: 11px;
  fill: #f3f4f6;
  stroke: rgba(10, 12, 16, 0.92);
  stroke-width: 2.8px;
  paint-order: stroke fill;
  stroke-linejoin: round;
  text-anchor: middle;
  dominant-baseline: central;
  font-weight: 600;
  letter-spacing: 0.03em;
}

.point-label {
  font-size: 14px;
  font-weight: 600;
  dominant-baseline: central;
  stroke: rgba(10, 12, 16, 0.95);
  stroke-width: 3.4px;
  paint-order: stroke fill;
  stroke-linejoin: round;
}

.player-name-tag {
  font-size: 12px;
  font-weight: 700;
  fill: #ffd166;
  dominant-baseline: central;
  stroke: rgba(10, 12, 16, 0.96);
  stroke-width: 3.5px;
  paint-order: stroke fill;
  stroke-linejoin: round;
}

.teammate-name-tag {
  font-size: 12px;
  font-weight: 700;
  fill: #38bdf8;
  dominant-baseline: central;
  stroke: rgba(10, 12, 16, 0.96);
  stroke-width: 3.5px;
  paint-order: stroke fill;
  stroke-linejoin: round;
}

.points-legend {
  position: absolute;
  right: 16px;
  bottom: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 12px;
  border-radius: 8px;
  background: rgba(24, 27, 34, 0.88);
  border: 1px solid var(--tarkov-border);
  backdrop-filter: blur(8px);
  font-size: 12px;
  color: var(--tarkov-text-muted);
  z-index: 15;
  pointer-events: none;
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.dot-pmc {
  background-color: #22c55e;
}

.dot-scav {
  background-color: #f59e0b;
}

.dot-shared {
  background-color: #38bdf8;
}

.diamond {
  width: 8px;
  height: 8px;
  background-color: #c084fc;
  transform: rotate(45deg);
  display: inline-block;
}

.player-pulse {
  fill: none;
  stroke: var(--tarkov-gold);
  stroke-width: 2.5;
  animation: pulse-ring 1.1s ease-out forwards;
}

@keyframes pulse-ring {
  0% {
    r: 7;
    opacity: 0.95;
  }
  100% {
    r: 34;
    opacity: 0;
  }
}

.canvas-overlay-msg {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--tarkov-text-muted);
  font-size: 14px;
  z-index: 10;
  pointer-events: none;
}

.canvas-overlay-msg.is-error {
  color: var(--tarkov-status-err);
}

.layer-switcher {
  position: absolute;
  left: 16px;
  bottom: 16px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 6px;
  border-radius: 8px;
  background: rgba(24, 27, 34, 0.88);
  border: 1px solid var(--tarkov-border);
  backdrop-filter: blur(8px);
  z-index: 15;
}

.layer-btn {
  padding: 5px 10px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--tarkov-text-muted);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.layer-btn:hover {
  color: var(--tarkov-text-primary);
  background: rgba(255, 255, 255, 0.06);
}

.layer-btn.is-active {
  color: var(--tarkov-gold);
  background: var(--tarkov-gold-soft);
  border-color: rgba(212, 163, 89, 0.45);
  font-weight: 600;
}
</style>
