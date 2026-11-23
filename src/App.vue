<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ElMessage } from 'element-plus';
import TopToolbar from '@/components/TopToolbar.vue';
import MapCanvas from '@/components/MapCanvas.vue';
import SettingsModal from '@/components/SettingsModal.vue';
import { useTauriEvents } from '@/composables/useTauriEvents';
import { TARKOV_MAPS, getMapById } from '@/data/maps.config';
import type { AppSettings, ScreenshotCoord, WatcherStatus } from '@/types';

const canvasRef = ref<InstanceType<typeof MapCanvas> | null>(null);
const settingsVisible = ref(false);
const namePromptVisible = ref(false);
const nameInput = ref('');
const showLabels = ref(true);
const showPoints = ref(true);
const latestCoord = ref<ScreenshotCoord | null>(null);

const settings = ref<AppSettings>({
  playerName: '',
  screenshotDir: '',
  logsDir: '',
  autoDeleteScreenshot: false,
  maxScreenshotsKeep: 10,
  autoSwitchMapByLog: true,
  autoCenterOnCoord: true,
  lastSelectedMapId: 'customs',
});

const watcherStatus = ref<WatcherStatus>({
  screenshotWatching: false,
  logWatching: false,
});

const currentMap = computed(() => getMapById(settings.value.lastSelectedMapId));

async function persistSettings(next: AppSettings) {
  settings.value = { ...next };
  try {
    watcherStatus.value = await invoke<WatcherStatus>('save_settings', { settings: next });
  } catch (err) {
    ElMessage.error(`保存设置失败: ${String(err)}`);
  }
}

function handleMapSelect(mapId: string) {
  persistSettings({ ...settings.value, lastSelectedMapId: mapId });
}

function handleAutoSwitchToggle(val: boolean) {
  persistSettings({ ...settings.value, autoSwitchMapByLog: val });
}

function handleAutoCenterToggle(val: boolean) {
  persistSettings({ ...settings.value, autoCenterOnCoord: val });
}

useTauriEvents({
  onScreenshotCoord(payload) {
    latestCoord.value = payload;
  },
  onMapDetected(payload) {
    if (!settings.value.autoSwitchMapByLog) return;
    if (payload.mapId && payload.mapId !== settings.value.lastSelectedMapId) {
      const target = getMapById(payload.mapId);
      persistSettings({ ...settings.value, lastSelectedMapId: target.id });
      ElMessage.success(`已自动切换至地图: ${target.nameZh}`);
    }
  },
  onWatcherError(payload) {
    ElMessage.warning(payload.message);
  },
});

onMounted(async () => {
  try {
    settings.value = await invoke<AppSettings>('get_settings');
    watcherStatus.value = await invoke<WatcherStatus>('get_watcher_status');
    if (!settings.value.playerName?.trim()) {
      namePromptVisible.value = true;
    }
  } catch (err) {
    ElMessage.error(`初始化状态失败: ${String(err)}`);
  }
});

async function submitInitialName() {
  const trimmed = nameInput.value.trim();
  if (!trimmed) {
    ElMessage.warning('请输入一个自定义名称');
    return;
  }
  await persistSettings({ ...settings.value, playerName: trimmed });
  namePromptVisible.value = false;
}
</script>

<template>
  <div class="app-shell">
    <TopToolbar
      :maps="TARKOV_MAPS"
      :selected-map-id="settings.lastSelectedMapId"
      :auto-switch-map-by-log="settings.autoSwitchMapByLog"
      :auto-center-on-coord="settings.autoCenterOnCoord"
      :show-labels="showLabels"
      :show-points="showPoints"
      :latest-coord="latestCoord"
      :watcher-status="watcherStatus"
      :screenshot-dir="settings.screenshotDir"
      @update:selected-map-id="handleMapSelect"
      @update:auto-switch-map-by-log="handleAutoSwitchToggle"
      @update:auto-center-on-coord="handleAutoCenterToggle"
      @update:show-labels="showLabels = $event"
      @update:show-points="showPoints = $event"
      @open-settings="settingsVisible = true"
    />

    <main class="map-workspace">
      <MapCanvas
        ref="canvasRef"
        :map-config="currentMap"
        :latest-coord="latestCoord"
        :auto-center="settings.autoCenterOnCoord"
        :show-labels="showLabels"
        :show-points="showPoints"
        :player-name="settings.playerName"
      />
    </main>

    <SettingsModal
      v-model:visible="settingsVisible"
      :settings="settings"
      @save="persistSettings"
    />

    <el-dialog
      v-model="namePromptVisible"
      title="欢迎使用 TarkovMap"
      width="400px"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      :show-close="false"
    >
      <p class="prompt-desc">请输入你的自定义名称，截图定位时将显示在位置标记旁：</p>
      <el-input
        v-model="nameInput"
        maxlength="24"
        placeholder="例如：塔科夫老兵"
        autofocus
        @keyup.enter="submitInitialName"
      />
      <template #footer>
        <el-button type="primary" @click="submitInitialName">确认并进入</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.app-shell {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.map-workspace {
  position: relative;
  flex: 1;
  width: 100%;
  overflow: hidden;
  display: flex;
}

.prompt-desc {
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--tarkov-text-muted);
}
</style>
