<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ElMessage } from 'element-plus';
import TopToolbar from '@/components/TopToolbar.vue';
import MapCanvas from '@/components/MapCanvas.vue';
import SettingsModal from '@/components/SettingsModal.vue';
import TeamModal from '@/components/TeamModal.vue';
import { useTauriEvents } from '@/composables/useTauriEvents';
import { TARKOV_MAPS, getMapById } from '@/data/maps.config';
import type {
  AppSettings,
  ScreenshotCoord,
  ServerConnectionStatus,
  ServerRoomSummary,
  TeamRoomState,
  WatcherStatus,
} from '@/types';

const canvasRef = ref<InstanceType<typeof MapCanvas> | null>(null);
const settingsVisible = ref(false);
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

const currentMap = computed(() => getMapById(settings.value.lastSelectedMapId));

const teammates = computed(() =>
  teamState.value.members.filter((m) => m.peerId !== teamState.value.selfPeerId)
);

async function syncPositionToTeam() {
  if (!teamState.value.inRoom) return;
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
  try {
    watcherStatus.value = await invoke<WatcherStatus>('save_settings', { settings: next });
    void syncPositionToTeam();
  } catch (err) {
    ElMessage.error(`保存设置失败: ${String(err)}`);
  }
}

function handleMapSelect(mapId: string) {
  void persistSettings({ ...settings.value, lastSelectedMapId: mapId });
}

function handleAutoSwitchToggle(val: boolean) {
  void persistSettings({ ...settings.value, autoSwitchMapByLog: val });
}

function handleAutoCenterToggle(val: boolean) {
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
});

onMounted(async () => {
  try {
    settings.value = await invoke<AppSettings>('get_settings');
    watcherStatus.value = await invoke<WatcherStatus>('get_watcher_status');
    teamState.value = await invoke<TeamRoomState>('get_team_state');
    serverStatus.value = await invoke<ServerConnectionStatus>('server_get_status');
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
      :team-state="teamState"
      @update:selected-map-id="handleMapSelect"
      @update:auto-switch-map-by-log="handleAutoSwitchToggle"
      @update:auto-center-on-coord="handleAutoCenterToggle"
      @update:show-labels="showLabels = $event"
      @update:show-points="showPoints = $event"
      @open-settings="settingsVisible = true"
      @open-team="teamModalVisible = true"
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
        :teammates="teammates"
      />
    </main>

    <SettingsModal
      v-model:visible="settingsVisible"
      :settings="settings"
      @save="persistSettings"
    />

    <TeamModal
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
