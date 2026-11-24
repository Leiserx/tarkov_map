<script setup lang="ts">
import { Close, CopyDocument, Minus, Setting, UserFilled } from '@element-plus/icons-vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ScreenshotCoord, TarkovMapConfig, TeamRoomState, WatcherStatus } from '@/types';

defineProps<{
  maps: TarkovMapConfig[];
  selectedMapId: string;
  autoSwitchMapByLog: boolean;
  autoCenterOnCoord: boolean;
  showLabels: boolean;
  showPoints: boolean;
  latestCoord: ScreenshotCoord | null;
  watcherStatus: WatcherStatus;
  screenshotDir: string;
  teamState: TeamRoomState;
  isBrowserMode: boolean;
  lanConnected: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:selectedMapId', val: string): void;
  (e: 'update:autoSwitchMapByLog', val: boolean): void;
  (e: 'update:autoCenterOnCoord', val: boolean): void;
  (e: 'update:showLabels', val: boolean): void;
  (e: 'update:showPoints', val: boolean): void;
  (e: 'openSettings'): void;
  (e: 'openTeam'): void;
}>();

const appWindow =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
    ? getCurrentWindow()
    : null;

function handleMinimize() {
  void appWindow?.minimize();
}

function handleToggleMaximize() {
  void appWindow?.toggleMaximize();
}

function handleClose() {
  void appWindow?.close();
}
</script>

<template>
  <header class="top-toolbar" data-tauri-drag-region>
    <div class="toolbar-left" data-tauri-drag-region>
      <div class="brand-badge" data-tauri-drag-region>TKFMAP</div>

      <el-select
        :model-value="selectedMapId"
        class="map-select"
        placeholder="选择地图"
        @update:model-value="emit('update:selectedMapId', $event)"
      >
        <el-option
          v-for="m in maps"
          :key="m.id"
          :label="m.nameZh"
          :value="m.id"
        />
      </el-select>

      <el-tooltip
        v-if="!isBrowserMode"
        content="开启后将根据游戏日志（含离线/PVE）自动切换进入的地图"
        placement="bottom"
      >
        <button
          type="button"
          class="toolbar-chip"
          :class="{ 'is-active': autoSwitchMapByLog }"
          @click="emit('update:autoSwitchMapByLog', !autoSwitchMapByLog)"
        >
          <span class="chip-dot" :class="autoSwitchMapByLog ? 'is-gold' : 'is-muted'" />
          <span>{{ autoSwitchMapByLog ? '日志自动切图' : '手动锁定地图' }}</span>
        </button>
      </el-tooltip>

      <div v-else class="toolbar-chip is-active">
        <span class="chip-dot" :class="lanConnected ? 'is-ok' : 'is-err'" />
        <span>{{ lanConnected ? '局域网同步中' : '局域网重连中' }}</span>
      </div>
    </div>

    <div class="toolbar-center mono-text" data-tauri-drag-region>
      <template v-if="latestCoord">
        <span class="coord-item"><b>X</b>{{ latestCoord.x.toFixed(1) }}</span>
        <span class="coord-sep">|</span>
        <span class="coord-item"><b>Y</b>{{ latestCoord.y.toFixed(1) }}</span>
        <span class="coord-sep">|</span>
        <span class="coord-item"><b>Z</b>{{ latestCoord.z.toFixed(1) }}</span>
      </template>
      <span v-else class="coord-placeholder">等待游戏内截图 (PrintScreen)...</span>
    </div>

    <div class="toolbar-right">
      <el-tooltip content="显示或隐藏 PMC / Scav 撤离点与转移点" placement="bottom">
        <div class="switch-chip">
          <span class="switch-label" :class="{ 'is-on': showPoints }">撤离/转移</span>
          <el-switch
            :model-value="showPoints"
            size="small"
            @update:model-value="emit('update:showPoints', Boolean($event))"
          />
        </div>
      </el-tooltip>

      <el-tooltip content="显示或隐藏地图中文地区名称" placement="bottom">
        <div class="switch-chip">
          <span class="switch-label" :class="{ 'is-on': showLabels }">地区名</span>
          <el-switch
            :model-value="showLabels"
            size="small"
            @update:model-value="emit('update:showLabels', Boolean($event))"
          />
        </div>
      </el-tooltip>

      <el-tooltip content="收到新截图坐标时自动居中画布" placement="bottom">
        <div class="switch-chip">
          <span class="switch-label" :class="{ 'is-on': autoCenterOnCoord }">居中跟随</span>
          <el-switch
            :model-value="autoCenterOnCoord"
            size="small"
            @update:model-value="emit('update:autoCenterOnCoord', Boolean($event))"
          />
        </div>
      </el-tooltip>

      <template v-if="!isBrowserMode">
        <el-tooltip
          :content="
            watcherStatus.screenshotWatching
              ? `正在监听: ${screenshotDir}`
              : watcherStatus.screenshotError || '截图目录未就绪'
          "
          placement="bottom"
        >
          <button type="button" class="toolbar-chip" @click="emit('openSettings')">
            <span
              class="chip-dot"
              :class="watcherStatus.screenshotWatching ? 'is-ok' : 'is-err'"
            />
            <span>{{ watcherStatus.screenshotWatching ? '监听中' : '路径异常' }}</span>
          </button>
        </el-tooltip>

        <button
          type="button"
          class="toolbar-chip"
          :class="{ 'is-active': teamState.inRoom }"
          @click="emit('openTeam')"
        >
          <el-icon :size="14"><UserFilled /></el-icon>
          <span v-if="teamState.inRoom">
            {{ teamState.roomCode }} ({{ teamState.memberCount }}人)
          </span>
          <span v-else>组队</span>
        </button>

        <button type="button" class="toolbar-chip" @click="emit('openSettings')">
          <el-icon :size="14"><Setting /></el-icon>
          <span>设置</span>
        </button>

        <div class="window-controls">
          <button type="button" class="win-btn" title="最小化" @click="handleMinimize">
            <el-icon :size="13"><Minus /></el-icon>
          </button>
          <button type="button" class="win-btn" title="最大化 / 还原" @click="handleToggleMaximize">
            <el-icon :size="12"><CopyDocument /></el-icon>
          </button>
          <button type="button" class="win-btn win-btn-close" title="关闭" @click="handleClose">
            <el-icon :size="13"><Close /></el-icon>
          </button>
        </div>
      </template>
    </div>
  </header>
</template>

<style scoped>
.top-toolbar {
  height: 48px;
  padding: 0 10px 0 14px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  background-color: var(--tarkov-bg-panel);
  border-bottom: 1px solid var(--tarkov-border);
  flex-shrink: 0;
  z-index: 20;
}

.toolbar-left,
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.brand-badge {
  height: 28px;
  display: inline-flex;
  align-items: center;
  padding: 0 10px;
  font-weight: 700;
  font-size: 12px;
  letter-spacing: 0.08em;
  color: var(--tarkov-gold);
  border: 1px solid rgba(212, 163, 89, 0.45);
  border-radius: var(--tarkov-radius);
  background: var(--tarkov-gold-soft);
}

.map-select {
  width: 136px;
}

.map-select :deep(.el-select__wrapper) {
  height: 28px;
  min-height: 28px;
  background-color: var(--tarkov-bg-control);
  box-shadow: 0 0 0 1px var(--tarkov-border) inset;
  border-radius: var(--tarkov-radius);
  font-size: 12px;
}

.map-select :deep(.el-select__wrapper:hover),
.map-select :deep(.el-select__wrapper.is-focused) {
  box-shadow: 0 0 0 1px var(--tarkov-border-hover) inset;
}

.toolbar-chip {
  height: 28px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  border-radius: var(--tarkov-radius);
  border: 1px solid var(--tarkov-border);
  background: var(--tarkov-bg-control);
  color: var(--tarkov-text-muted);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.toolbar-chip:hover {
  border-color: var(--tarkov-border-hover);
  color: var(--tarkov-text-primary);
}

.toolbar-chip.is-active {
  color: var(--tarkov-gold);
  background: var(--tarkov-gold-soft);
  border-color: rgba(212, 163, 89, 0.45);
}

.chip-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.chip-dot.is-ok {
  background-color: var(--tarkov-status-ok);
  box-shadow: 0 0 6px var(--tarkov-status-ok);
}

.chip-dot.is-err {
  background-color: var(--tarkov-status-err);
  box-shadow: 0 0 6px var(--tarkov-status-err);
}

.chip-dot.is-gold {
  background-color: var(--tarkov-gold);
}

.chip-dot.is-muted {
  background-color: #6b7280;
}

.toolbar-center {
  height: 28px;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  font-size: 12px;
  color: var(--tarkov-text-primary);
  background: var(--tarkov-bg-control);
  border-radius: var(--tarkov-radius);
  border: 1px solid var(--tarkov-border);
  white-space: nowrap;
}

.coord-item b {
  color: var(--tarkov-gold);
  font-weight: 600;
  margin-right: 4px;
}

.coord-sep {
  color: var(--tarkov-border);
}

.coord-placeholder {
  color: var(--tarkov-text-muted);
}

.switch-chip {
  height: 28px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  border-radius: var(--tarkov-radius);
  border: 1px solid var(--tarkov-border);
  background: var(--tarkov-bg-control);
  transition: border-color 0.15s ease;
  white-space: nowrap;
}

.switch-chip:hover {
  border-color: var(--tarkov-border-hover);
}

.switch-label {
  font-size: 12px;
  color: var(--tarkov-text-muted);
  transition: color 0.15s ease;
}

.switch-label.is-on {
  color: var(--tarkov-text-primary);
}

.window-controls {
  display: inline-flex;
  align-items: center;
  margin-left: 4px;
  padding-left: 8px;
  border-left: 1px solid var(--tarkov-border);
  gap: 2px;
}

.win-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--tarkov-radius);
  background: transparent;
  color: var(--tarkov-text-muted);
  cursor: pointer;
  transition: all 0.15s ease;
}

.win-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--tarkov-text-primary);
}

.win-btn-close:hover {
  background: rgba(239, 68, 68, 0.85);
  color: #ffffff;
}
</style>
