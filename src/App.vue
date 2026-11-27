<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ElMessage } from 'element-plus';
import TopToolbar from '@/components/TopToolbar.vue';
import MapCanvas from '@/components/MapCanvas.vue';
import MapObjectivesPanel from '@/components/MapObjectivesPanel.vue';
import MapObjectiveDetails from '@/components/MapObjectiveDetails.vue';
import SettingsModal from '@/components/SettingsModal.vue';
import GammaModal from '@/components/GammaModal.vue';
import TeamModal from '@/components/TeamModal.vue';
import { useTauriEvents } from '@/composables/useTauriEvents';
import { TARKOV_MAPS, getMapById } from '@/data/maps.config';
import mapObjectives from '@/data/map-objectives.json';
import reserveObjectives from '@/data/reserve-objectives.json';
import type {
  AppSettings,
  LanServerStatus,
  LanSnapshot,
  MapObjectiveEntry,
  MapObjectiveLocation,
  ScreenshotCoord,
  ServerConnectionStatus,
  ServerRoomSummary,
  TeamMemberInfo,
  TeamRoomState,
  WatcherStatus,
} from '@/types';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const canvasRef = ref<InstanceType<typeof MapCanvas> | null>(null);
const settingsVisible = ref(false);
const gammaVisible = ref(false);
const teamModalVisible = ref(false);
const teamLoading = ref(false);
const namePromptVisible = ref(false);
const nameInput = ref('');
const showLabels = ref(true);
const showPoints = ref(true);
const latestCoord = ref<ScreenshotCoord | null>(null);

const settings = ref<AppSettings>({
  playerName: '',
  lastServerAddr: '127.0.0.1:8765',
  lanShareEnabled: false,
  lanSharePort: 18765,
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

const teamState = ref<TeamRoomState>({
  inRoom: false,
  isHost: false,
  mode: 'p2p',
  selfPeerId: '',
  roomCode: '',
  fullCode: '',
  memberCount: 0,
  members: [],
});

const serverStatus = ref<ServerConnectionStatus>({
  connected: false,
  addr: '',
});

const serverRooms = ref<ServerRoomSummary[]>([]);

const lanStatus = ref<LanServerStatus>({
  running: false,
  url: '',
});

const lanBrowserConnected = ref(false);
const lanTeammates = ref<TeamMemberInfo[]>([]);

const currentMap = computed(() => getMapById(settings.value.lastSelectedMapId));
const objectivesSidebarKey = 'tkfmap.objectivesSidebar.collapsed';
const objectivesCollapsed = ref(true);
try {
  objectivesCollapsed.value = localStorage.getItem(objectivesSidebarKey) !== 'false';
} catch {
  // The sidebar can still be toggled when browser storage is unavailable.
}
watch(objectivesCollapsed, (collapsed) => {
  try {
    localStorage.setItem(objectivesSidebarKey, String(collapsed));
  } catch {
    // Keep the preference for the current session.
  }
});

const objectivesByMap: Record<string, MapObjectiveEntry[]> = {
  ...mapObjectives as Record<string, MapObjectiveEntry[]>,
  reserve: reserveObjectives as MapObjectiveEntry[],
};
const objectiveEntries = computed(() => objectivesByMap[currentMap.value.id] ?? []);
const selectedObjectives = ref<MapObjectiveLocation[] | null>([]);
const focusedObjectives = ref<MapObjectiveLocation[] | null>(null);
const detailEntry = ref<MapObjectiveEntry | null>(null);
const detailLocation = ref<MapObjectiveLocation | null>(null);
const visibleObjectives = computed(() =>
  selectedObjectives.value ?? objectiveEntries.value.flatMap((entry) => entry.locations)
);

watch(() => currentMap.value.id, () => {
  selectedObjectives.value = [];
  focusedObjectives.value = null;
  closeObjectiveDetails();
});

function updateObjectives(locations: MapObjectiveLocation[] | null, focus: MapObjectiveLocation[] | null) {
  selectedObjectives.value = locations;
  focusedObjectives.value = focus;
}

function openObjectiveDetails(entry: MapObjectiveEntry, location: MapObjectiveLocation | null = null) {
  detailEntry.value = entry;
  detailLocation.value = location;
}

function openObjectiveLocation(locationId: string) {
  const entry = objectiveEntries.value.find((item) => item.locations.some((location) => location.id === locationId));
  if (entry) openObjectiveDetails(entry, entry.locations.find((location) => location.id === locationId));
}

function closeObjectiveDetails() {
  detailEntry.value = null;
  detailLocation.value = null;
}

const teammates = computed(() =>
  isTauri
    ? teamState.value.members.filter((m) => m.peerId !== teamState.value.selfPeerId)
    : lanTeammates.value
);

function pushLanSnapshot() {
  if (!isTauri) return;
  const snapshot: LanSnapshot = {
    mapId: settings.value.lastSelectedMapId,
    playerName: settings.value.playerName,
    latestCoord: latestCoord.value,
    teammates: teammates.value,
  };
  void invoke('update_lan_snapshot', { snapshot }).catch(() => {});
}

watch(
  [
    () => settings.value.lastSelectedMapId,
    () => settings.value.playerName,
    latestCoord,
    teammates,
  ],
  () => {
    pushLanSnapshot();
  },
  { deep: true }
);

async function syncPositionToTeam() {
  if (!isTauri || !teamState.value.inRoom) return;
  try {
    await invoke('sync_team_position', {
      playerName: settings.value.playerName || '未命名玩家',
      mapId: settings.value.lastSelectedMapId,
      x: latestCoord.value?.x ?? null,
      y: latestCoord.value?.y ?? null,
      z: latestCoord.value?.z ?? null,
    });
  } catch {
    // 忽略瞬时同步失败
  }
}

async function persistSettings(next: AppSettings) {
  settings.value = { ...next };
  if (!isTauri) return;
  try {
    watcherStatus.value = await invoke<WatcherStatus>('save_settings', { settings: next });
    void syncPositionToTeam();
  } catch (err) {
    ElMessage.error(`保存设置失败: ${String(err)}`);
  }
}

function handleMapSelect(mapId: string) {
  if (!isTauri) {
    settings.value.lastSelectedMapId = mapId;
    return;
  }
  void persistSettings({ ...settings.value, lastSelectedMapId: mapId });
}

function handleAutoSwitchToggle(val: boolean) {
  void persistSettings({ ...settings.value, autoSwitchMapByLog: val });
}

function handleAutoCenterToggle(val: boolean) {
  if (!isTauri) {
    settings.value.autoCenterOnCoord = val;
    return;
  }
  void persistSettings({ ...settings.value, autoCenterOnCoord: val });
}

async function handleCreateRoom() {
  teamLoading.value = true;
  try {
    teamState.value = await invoke<TeamRoomState>('create_room', {
      playerName: settings.value.playerName || '未命名玩家',
      mapId: settings.value.lastSelectedMapId,
    });
    ElMessage.success(`P2P 房间已创建，Code: ${teamState.value.roomCode}`);
    void syncPositionToTeam();
  } catch (err) {
    ElMessage.error(`创建房间失败: ${String(err)}`);
  } finally {
    teamLoading.value = false;
  }
}

async function handleJoinRoom(code: string) {
  teamLoading.value = true;
  try {
    teamState.value = await invoke<TeamRoomState>('join_room', {
      code,
      playerName: settings.value.playerName || '未命名玩家',
      mapId: settings.value.lastSelectedMapId,
    });
    ElMessage.success('已成功加入 P2P 房间');
    void syncPositionToTeam();
  } catch (err) {
    ElMessage.error(`加入房间失败: ${String(err)}`);
  } finally {
    teamLoading.value = false;
  }
}

async function handleLeaveRoom() {
  teamLoading.value = true;
  try {
    teamState.value = await invoke<TeamRoomState>('leave_room');
    ElMessage.info('已退出组队房间');
  } catch (err) {
    ElMessage.error(`退出房间失败: ${String(err)}`);
  } finally {
    teamLoading.value = false;
  }
}

async function handleServerConnect(addr: string) {
  teamLoading.value = true;
  try {
    serverStatus.value = await invoke<ServerConnectionStatus>('server_connect', { addr });
    void persistSettings({ ...settings.value, lastServerAddr: addr.trim() });
    ElMessage.success('已连接到联机服务器');
  } catch (err) {
    ElMessage.error(String(err));
  } finally {
    teamLoading.value = false;
  }
}

async function handleServerDisconnect() {
  await invoke('server_disconnect');
  ElMessage.info('已断开服务器连接');
}

async function handleServerRefreshRooms() {
  try {
    await invoke('server_list_rooms');
  } catch (err) {
    ElMessage.warning(String(err));
  }
}

async function handleServerCreateRoom(payload: { roomName: string; passcode: string }) {
  teamLoading.value = true;
  try {
    await invoke('server_create_room', {
      roomName: payload.roomName,
      passcode: payload.passcode,
      playerName: settings.value.playerName || '未命名玩家',
      mapId: settings.value.lastSelectedMapId,
    });
  } catch (err) {
    ElMessage.error(String(err));
  } finally {
    teamLoading.value = false;
  }
}

async function handleServerJoinRoom(payload: { roomId: string; passcode: string }) {
  teamLoading.value = true;
  try {
    await invoke('server_join_room', {
      roomId: payload.roomId,
      passcode: payload.passcode,
      playerName: settings.value.playerName || '未命名玩家',
      mapId: settings.value.lastSelectedMapId,
    });
  } catch (err) {
    ElMessage.error(String(err));
  } finally {
    teamLoading.value = false;
  }
}

useTauriEvents({
  onScreenshotCoord(payload) {
    latestCoord.value = payload;
    void syncPositionToTeam();
  },
  onMapDetected(payload) {
    if (!settings.value.autoSwitchMapByLog) return;
    if (payload.mapId && payload.mapId !== settings.value.lastSelectedMapId) {
      const target = getMapById(payload.mapId);
      void persistSettings({ ...settings.value, lastSelectedMapId: target.id });
      ElMessage.success(`已自动切换至地图: ${target.nameZh}`);
    }
  },
  onWatcherError(payload) {
    ElMessage.warning(payload.message);
  },
  onTeamStateUpdated(payload) {
    const justJoined = !teamState.value.inRoom && payload.inRoom;
    teamState.value = payload;
    if (justJoined) {
      void syncPositionToTeam();
    }
  },
  onTeamRoomClosed(reason) {
    ElMessage.warning(reason);
  },
  onServerConnectionChanged(payload) {
    serverStatus.value = payload;
  },
  onServerRoomsUpdated(payload) {
    serverRooms.value = payload;
  },
  onServerError(message) {
    ElMessage.error(message);
  },
  onLanStatusChanged(payload) {
    lanStatus.value = payload;
  },
});

let lanWs: WebSocket | null = null;
let lanReconnectTimer: number | null = null;
let unmounted = false;

function connectLanBrowserWs() {
  if (unmounted || isTauri) return;
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  lanWs = new WebSocket(`${proto}//${location.host}/ws`);

  lanWs.onopen = () => {
    lanBrowserConnected.value = true;
  };

  lanWs.onmessage = (event) => {
    try {
      const snap = JSON.parse(String(event.data)) as LanSnapshot;
      if (snap.mapId) {
        settings.value.lastSelectedMapId = snap.mapId;
      }
      settings.value.playerName = snap.playerName ?? '';
      latestCoord.value = snap.latestCoord ?? null;
      lanTeammates.value = snap.teammates ?? [];
    } catch {
      // ignore invalid frame
    }
  };

  lanWs.onclose = () => {
    lanBrowserConnected.value = false;
    if (!unmounted) {
      lanReconnectTimer = window.setTimeout(connectLanBrowserWs, 3000);
    }
  };
}

onMounted(async () => {
  if (!isTauri) {
    connectLanBrowserWs();
    return;
  }

  try {
    settings.value = await invoke<AppSettings>('get_settings');
    watcherStatus.value = await invoke<WatcherStatus>('get_watcher_status');
    teamState.value = await invoke<TeamRoomState>('get_team_state');
    serverStatus.value = await invoke<ServerConnectionStatus>('server_get_status');
    lanStatus.value = await invoke<LanServerStatus>('get_lan_status');
    pushLanSnapshot();
    if (!settings.value.playerName?.trim()) {
      namePromptVisible.value = true;
    }
  } catch (err) {
    ElMessage.error(`初始化状态失败: ${String(err)}`);
  }
});

onUnmounted(() => {
  unmounted = true;
  if (lanReconnectTimer !== null) {
    clearTimeout(lanReconnectTimer);
  }
  lanWs?.close();
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
      :team-state="teamState"
      :is-browser-mode="!isTauri"
      :lan-connected="lanBrowserConnected"
      @update:selected-map-id="handleMapSelect"
      @update:auto-switch-map-by-log="handleAutoSwitchToggle"
      @update:auto-center-on-coord="handleAutoCenterToggle"
      @update:show-labels="showLabels = $event"
      @update:show-points="showPoints = $event"
      @open-settings="settingsVisible = true"
      @open-gamma="gammaVisible = true"
      @open-team="teamModalVisible = true"
    />

    <main class="map-workspace">
      <MapObjectivesPanel
        v-if="currentMap.id in objectivesByMap"
        :key="currentMap.id"
        v-model:collapsed="objectivesCollapsed"
        :entries="objectiveEntries"
        @select="updateObjectives"
        @details="openObjectiveDetails"
      />
      <MapCanvas
        ref="canvasRef"
        :map-config="currentMap"
        :latest-coord="latestCoord"
        :auto-center="settings.autoCenterOnCoord"
        :show-labels="showLabels"
        :show-points="showPoints"
        :player-name="settings.playerName"
        :teammates="teammates"
        :objective-locations="visibleObjectives"
        :focused-objectives="focusedObjectives"
        @objective-details="openObjectiveLocation"
      />
    </main>

    <MapObjectiveDetails
      :entry="detailEntry"
      :location="detailLocation"
      @close="closeObjectiveDetails"
      @select="detailLocation = $event"
    />

    <GammaModal v-if="isTauri" v-model:visible="gammaVisible" />

    <SettingsModal
      v-if="isTauri"
      v-model:visible="settingsVisible"
      :settings="settings"
      :lan-status="lanStatus"
      @save="persistSettings"
    />

    <TeamModal
      v-if="isTauri"
      v-model:visible="teamModalVisible"
      :team-state="teamState"
      :server-status="serverStatus"
      :server-rooms="serverRooms"
      :default-server-addr="settings.lastServerAddr"
      :loading="teamLoading"
      @create-room="handleCreateRoom"
      @join-room="handleJoinRoom"
      @leave-room="handleLeaveRoom"
      @server-connect="handleServerConnect"
      @server-disconnect="handleServerDisconnect"
      @server-refresh-rooms="handleServerRefreshRooms"
      @server-create-room="handleServerCreateRoom"
      @server-join-room="handleServerJoinRoom"
    />

    <el-dialog
      v-if="isTauri"
      v-model="namePromptVisible"
      title="欢迎使用 TarkovMap"
      width="400px"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      :show-close="false"
    >
      <p class="prompt-desc">请输入你的自定义名称，截图定位与组队时将显示在位置标记旁：</p>
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
  min-height: 0;
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
