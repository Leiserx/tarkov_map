<script setup lang="ts">
import { ref, watch } from 'vue';
import { ElMessage } from 'element-plus';
import { getMapById } from '@/data/maps.config';
import type {
  ServerConnectionStatus,
  ServerRoomSummary,
  TeamRoomState,
} from '@/types';

const props = defineProps<{
  visible: boolean;
  teamState: TeamRoomState;
  serverStatus: ServerConnectionStatus;
  serverRooms: ServerRoomSummary[];
  defaultServerAddr: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:visible', val: boolean): void;
  (e: 'createRoom'): void;
  (e: 'joinRoom', code: string): void;
  (e: 'leaveRoom'): void;
  (e: 'serverConnect', addr: string): void;
  (e: 'serverDisconnect'): void;
  (e: 'serverRefreshRooms'): void;
  (e: 'serverCreateRoom', payload: { roomName: string; passcode: string }): void;
  (e: 'serverJoinRoom', payload: { roomId: string; passcode: string }): void;
}>();

const activeTab = ref<'p2p' | 'server'>('p2p');
const joinCodeInput = ref('');

const serverAddrInput = ref(props.defaultServerAddr);
const createRoomName = ref('');
const createPasscode = ref('');
const joiningRoomId = ref('');
const joinPasscode = ref('');

watch(
  () => props.visible,
  (isOpen) => {
    if (isOpen) {
      serverAddrInput.value = props.serverStatus.connected
        ? props.serverStatus.addr
        : props.defaultServerAddr;
      if (props.teamState.inRoom && props.teamState.mode === 'server') {
        activeTab.value = 'server';
      } else if (props.teamState.inRoom) {
        activeTab.value = 'p2p';
      }
      if (props.serverStatus.connected && !props.teamState.inRoom) {
        emit('serverRefreshRooms');
      }
    }
  }
);

async function copyText(text: string, label: string) {
  try {
    await navigator.clipboard.writeText(text);
    ElMessage.success(`已复制${label}`);
  } catch {
    ElMessage.warning('复制失败，请手动选中复制');
  }
}

function handleJoinP2p() {
  const code = joinCodeInput.value.trim();
  if (!code) {
    ElMessage.warning('请输入房间 Code');
    return;
  }
  emit('joinRoom', code);
}

function handleServerCreate() {
  if (!createPasscode.value.trim()) {
    ElMessage.warning('请设置房间口令');
    return;
  }
  emit('serverCreateRoom', {
    roomName: createRoomName.value.trim(),
    passcode: createPasscode.value.trim(),
  });
}

function startJoinServerRoom(roomId: string) {
  joiningRoomId.value = roomId;
  joinPasscode.value = '';
}

function confirmJoinServerRoom(roomId: string) {
  if (!joinPasscode.value.trim()) {
    ElMessage.warning('请输入房间口令');
    return;
  }
  emit('serverJoinRoom', {
    roomId,
    passcode: joinPasscode.value.trim(),
  });
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    title="组队共享位置"
    width="500px"
    @update:model-value="emit('update:visible', $event)"
  >
    <!-- 已在房间内：统一展示房间成员、人数与退出操作 -->
    <template v-if="teamState.inRoom">
      <div class="room-header-card">
        <div class="room-meta">
          <el-tag size="small" type="info" effect="dark">
            {{ teamState.mode === 'server' ? '服务器联机' : 'P2P 直连' }}
          </el-tag>
          <span class="meta-code mono-text">
            {{ teamState.mode === 'server' ? teamState.fullCode : teamState.roomCode }}
          </span>
          <el-tag size="small" :type="teamState.isHost ? 'warning' : 'info'" effect="dark">
            {{ teamState.isHost ? '房主' : '队员' }}
          </el-tag>
        </div>

        <div v-if="teamState.mode !== 'server'" class="room-actions">
          <el-button size="small" @click="copyText(teamState.roomCode, '房间 Code')">
            复制 Code
          </el-button>
          <el-button
            size="small"
            plain
            @click="copyText(teamState.fullCode, '完整邀请码（备用）')"
          >
            复制备用码
          </el-button>
        </div>
        <div v-else class="room-actions">
          <span class="room-id-badge mono-text">ID: {{ teamState.roomCode }}</span>
        </div>
      </div>

      <div class="members-header">
        <span>当前房间人数</span>
        <span class="member-count">{{ teamState.memberCount }} 人在线</span>
      </div>

      <div class="members-list">
        <div v-for="m in teamState.members" :key="m.peerId" class="member-row">
          <div class="member-left">
            <span
              class="member-dot"
              :class="m.peerId === teamState.selfPeerId ? 'is-self' : 'is-peer'"
            />
            <span class="member-name">
              {{ m.playerName }}
              <em v-if="m.peerId === teamState.selfPeerId" class="self-tag">（我）</em>
            </span>
          </div>
          <div class="member-right">
            <span class="member-map">{{ getMapById(m.mapId).nameZh }}</span>
            <span class="member-coord-status">
              {{ m.x !== null && m.z !== null ? '已定位' : '待截图' }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <!-- 未在房间内：切换 P2P 无服务器 vs IP+端口服务器联机 -->
    <template v-else>
      <el-tabs v-model="activeTab">
        <el-tab-pane label="P2P 组队（无服务器）" name="p2p">
          <div class="team-section">
            <div class="section-title">创建新房间</div>
            <p class="section-desc">
              自动生成 6 位随机房间 Code，队友输入即可穿透直连共享位置。
            </p>
            <el-button
              type="primary"
              :loading="loading"
              style="width: 100%"
              @click="emit('createRoom')"
            >
              创建 P2P 房间
            </el-button>
          </div>

          <el-divider>或</el-divider>

          <div class="team-section">
            <div class="section-title">加入已有房间</div>
            <div class="join-row">
              <el-input
                v-model="joinCodeInput"
                placeholder="输入 6 位房间 Code 或完整邀请码"
                @keyup.enter="handleJoinP2p"
              />
              <el-button type="primary" plain :loading="loading" @click="handleJoinP2p">
                加入
              </el-button>
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane label="服务器联机（IP:端口）" name="server">
          <div class="team-section">
            <div class="section-title">连接联机服务器</div>
            <div class="join-row">
              <el-input
                v-model="serverAddrInput"
                :disabled="serverStatus.connected"
                placeholder="例如 127.0.0.1:8765"
                @keyup.enter="emit('serverConnect', serverAddrInput)"
              />
              <el-button
                v-if="!serverStatus.connected"
                type="primary"
                :loading="loading"
                @click="emit('serverConnect', serverAddrInput)"
              >
                连接
              </el-button>
              <el-button v-else type="danger" plain @click="emit('serverDisconnect')">
                断开
              </el-button>
            </div>
          </div>

          <template v-if="serverStatus.connected">
            <el-divider />

            <div class="members-header">
              <span>服务器在线房间（{{ serverRooms.length }}）</span>
              <el-button size="small" text type="primary" @click="emit('serverRefreshRooms')">
                刷新列表
              </el-button>
            </div>

            <div v-if="serverRooms.length === 0" class="empty-rooms">
              当前暂无在线房间，可在下方创建一个新房间
            </div>

            <div v-else class="members-list" style="margin-bottom: 12px">
              <div v-for="room in serverRooms" :key="room.roomId" class="server-room-card">
                <div class="member-row" style="border: none; padding: 0">
                  <div class="member-left">
                    <span class="member-name">{{ room.roomName }}</span>
                    <span class="room-id-badge mono-text">{{ room.roomId }}</span>
                  </div>
                  <div class="member-right">
                    <span class="member-coord-status">房主: {{ room.hostName }}</span>
                    <span class="member-map">{{ room.memberCount }} 人</span>
                    <el-button
                      size="small"
                      type="primary"
                      plain
                      @click="startJoinServerRoom(room.roomId)"
                    >
                      加入
                    </el-button>
                  </div>
                </div>

                <div v-if="joiningRoomId === room.roomId" class="passcode-row">
                  <el-input
                    v-model="joinPasscode"
                    size="small"
                    type="password"
                    show-password
                    placeholder="请输入房间口令"
                    @keyup.enter="confirmJoinServerRoom(room.roomId)"
                  />
                  <el-button
                    size="small"
                    type="primary"
                    :loading="loading"
                    @click="confirmJoinServerRoom(room.roomId)"
                  >
                    确认进入
                  </el-button>
                </div>
              </div>
            </div>

            <el-divider>创建服务器房间</el-divider>

            <div class="create-server-form">
              <el-input v-model="createRoomName" placeholder="房间名称（选填，默认使用昵称）" />
              <div class="join-row">
                <el-input
                  v-model="createPasscode"
                  type="password"
                  show-password
                  placeholder="设置房间口令（必填）"
                  @keyup.enter="handleServerCreate"
                />
                <el-button type="primary" :loading="loading" @click="handleServerCreate">
                  创建并进入
                </el-button>
              </div>
            </div>
          </template>
        </el-tab-pane>
      </el-tabs>
    </template>

    <template #footer>
      <template v-if="teamState.inRoom">
        <el-button type="danger" plain :loading="loading" @click="emit('leaveRoom')">
          {{ teamState.isHost ? '解散房间' : '退出房间' }}
        </el-button>
      </template>
      <el-button @click="emit('update:visible', false)">关闭</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.team-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--tarkov-text-primary);
}

.section-desc {
  margin: 0;
  font-size: 12px;
  color: var(--tarkov-text-muted);
}

.join-row {
  display: flex;
  gap: 8px;
}

.room-header-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 12px;
  border-radius: var(--tarkov-radius);
  background: var(--tarkov-bg-control);
  border: 1px solid var(--tarkov-border);
  margin-bottom: 14px;
}

.room-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.meta-code {
  font-size: 16px;
  font-weight: 700;
  letter-spacing: 0.06em;
  color: var(--tarkov-gold);
}

.room-actions {
  display: flex;
  gap: 6px;
}

.room-id-badge {
  font-size: 11px;
  color: var(--tarkov-text-muted);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid var(--tarkov-border);
}

.members-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  color: var(--tarkov-text-muted);
  margin-bottom: 8px;
}

.member-count {
  color: var(--tarkov-gold);
  font-weight: 600;
}

.members-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 210px;
  overflow-y: auto;
}

.member-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  border-radius: var(--tarkov-radius);
  background: var(--tarkov-bg-control);
  border: 1px solid var(--tarkov-border);
  font-size: 12px;
}

.server-room-card {
  padding: 8px 10px;
  border-radius: var(--tarkov-radius);
  background: var(--tarkov-bg-control);
  border: 1px solid var(--tarkov-border);
}

.passcode-row {
  display: flex;
  gap: 8px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px dashed var(--tarkov-border);
}

.create-server-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.empty-rooms {
  text-align: center;
  padding: 14px;
  font-size: 12px;
  color: var(--tarkov-text-muted);
  border: 1px dashed var(--tarkov-border);
  border-radius: var(--tarkov-radius);
}

.member-left,
.member-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.member-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.member-dot.is-self {
  background-color: #ff3b30;
}

.member-dot.is-peer {
  background-color: #38bdf8;
}

.member-name {
  color: var(--tarkov-text-primary);
  font-weight: 500;
}

.self-tag {
  font-style: normal;
  color: var(--tarkov-text-muted);
}

.member-map {
  color: var(--tarkov-gold);
}

.member-coord-status {
  color: var(--tarkov-text-muted);
}
</style>
