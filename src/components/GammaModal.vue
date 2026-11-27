<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ElMessage, ElMessageBox } from 'element-plus';

interface GammaProfile {
  id: string;
  name: string;
  monitorId: string;
  gamma: number;
  brightness: number;
  contrast: number;
  shortcut: string;
}

interface GammaStatus {
  profiles: GammaProfile[];
  displays: { id: string; label: string; primary: boolean }[];
  activeProfiles: Record<string, string>;
  warning: string | null;
}

type GammaAction =
  | { type: 'status' | 'reset' }
  | { type: 'save'; profiles: GammaProfile[] }
  | { type: 'apply'; id: string };

const visible = defineModel<boolean>('visible', { required: true });
const status = ref<GammaStatus>({ profiles: [], displays: [], activeProfiles: {}, warning: null });
const draft = ref<GammaProfile | null>(null);
const busy = ref(false);
const error = ref('');
const shortcutHint = ref('');
const editingSaved = computed(() => status.value.profiles.some((profile) => profile.id === draft.value?.id));

async function request(action: GammaAction): Promise<boolean> {
  if (busy.value) return false;
  busy.value = true;
  error.value = '';
  try {
    status.value = await invoke<GammaStatus>('gamma_tool', { action });
    return true;
  } catch (reason) {
    error.value = String(reason);
    return false;
  } finally {
    busy.value = false;
  }
}

watch(visible, async (open) => {
  if (open && await request({ type: 'status' })) {
    edit(status.value.profiles[0]);
  }
});

function edit(profile?: GammaProfile) {
  shortcutHint.value = '';
  draft.value = profile ? { ...profile } : {
    id: crypto.randomUUID(),
    name: '',
    monitorId: status.value.displays[0]?.id ?? '',
    gamma: 1,
    brightness: 0,
    contrast: 1,
    shortcut: '',
  };
}

function recordShortcut(event: KeyboardEvent) {
  if (event.key === 'Tab') return;
  event.preventDefault();
  event.stopPropagation();
  if (!draft.value || ['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return;
  const modifiers = [
    event.ctrlKey && 'Ctrl', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Win',
  ].filter(Boolean);
  const key = /^(Key[A-Z]|Digit[0-9])$/.test(event.code)
    ? event.code.slice(-1)
    : /^(F[1-9]|F10|F11)$/.test(event.code) ? event.code : '';
  if (!modifiers.length || !key) {
    shortcutHint.value = '请同时按下修饰键和字母、数字或 F1–F11';
    return;
  }
  shortcutHint.value = '';
  draft.value.shortcut = [...modifiers, key].join('+');
}

async function save() {
  if (!draft.value) return;
  const profile = { ...draft.value, name: draft.value.name.trim() };
  const profiles = editingSaved.value
    ? status.value.profiles.map((item) => item.id === profile.id ? profile : item)
    : [...status.value.profiles, profile];
  if (await request({ type: 'save', profiles })) {
    edit(profile);
    ElMessage.success('Gamma 配置已保存');
  }
}

async function remove() {
  const profile = draft.value;
  if (!profile) return;
  try {
    await ElMessageBox.confirm(`删除「${profile.name}」及其快捷键？`, '删除 Gamma 配置', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
  } catch {
    return;
  }
  const profiles = status.value.profiles.filter((item) => item.id !== profile.id);
  if (await request({ type: 'save', profiles })) edit(status.value.profiles[0]);
}

let unlisten: (() => void) | undefined;
let unmounted = false;
onMounted(async () => {
  try {
    const dispose = await listen<{ name: string; error: string | null; activeProfiles: Record<string, string> }>(
      'gamma-hotkey-result',
      ({ payload }) => {
        status.value.activeProfiles = payload.activeProfiles;
        if (payload.error) ElMessage.error(payload.error);
        else ElMessage.success(`Gamma 已切换到「${payload.name}」`);
      }
    );
    if (unmounted) dispose();
    else unlisten = dispose;
  } catch (reason) {
    error.value = String(reason);
  }
});
onUnmounted(() => {
  unmounted = true;
  unlisten?.();
});
</script>

<template>
  <el-dialog v-model="visible" title="小蝴蝶 · 显示器调节" width="min(650px, 94vw)">
    <p class="gamma-note">建议右键 TarkovMap 选择「以管理员身份运行」。画面调节适用于支持此功能的 Windows SDR 显示器。</p>
    <el-alert v-if="status.warning" :title="status.warning" type="warning" :closable="false" show-icon />
    <el-alert v-if="error" :title="error" type="error" :closable="false" show-icon />
    <div class="gamma-layout" v-loading="busy">
      <div class="profiles" aria-label="已保存的配置">
        <el-button @click="edit()">新建配置</el-button>
        <button
          v-for="profile in status.profiles"
          :key="profile.id"
          type="button"
          class="profile"
          :aria-pressed="draft?.id === profile.id"
          @click="edit(profile)"
        >
          <strong>{{ profile.name }}</strong>
          <small>Gamma {{ profile.gamma.toFixed(2) }} · 亮度 {{ Math.round(profile.brightness * 100) }}% · 对比度 {{ profile.contrast.toFixed(2) }}</small>
          <small v-if="profile.shortcut">{{ profile.shortcut }}</small>
          <small v-if="status.activeProfiles[profile.monitorId] === profile.id">已应用</small>
        </button>
      </div>
      <el-form v-if="draft" class="gamma-editor" label-position="top" @submit.prevent="save">
        <el-form-item label="配置名称">
          <el-input v-model="draft.name" maxlength="40" />
        </el-form-item>
        <el-form-item label="显示器">
          <el-select v-model="draft.monitorId" placeholder="选择显示器">
            <el-option v-for="display in status.displays" :key="display.id" :label="display.label" :value="display.id" />
          </el-select>
        </el-form-item>
        <el-form-item :label="`Gamma · ${draft.gamma.toFixed(2)}`">
          <el-slider v-model="draft.gamma" :min="0.7" :max="1.5" :step="0.01" aria-label="Gamma" />
        </el-form-item>
        <el-form-item :label="`亮度 Brightness · ${Math.round(draft.brightness * 100)}%`">
          <el-slider v-model="draft.brightness" :min="-0.15" :max="0.15" :step="0.01" aria-label="亮度 Brightness" />
        </el-form-item>
        <el-form-item :label="`对比度 Contrast · ${draft.contrast.toFixed(2)}`">
          <el-slider v-model="draft.contrast" :min="0.75" :max="1.25" :step="0.01" aria-label="对比度 Contrast" />
        </el-form-item>
        <el-form-item label="全局快捷键" :error="shortcutHint">
          <div class="shortcut">
            <input
              class="shortcut-input"
              :value="draft.shortcut"
              placeholder="点击后按下组合键"
              readonly
              aria-label="录入全局快捷键"
              @keydown="recordShortcut"
            />
            <el-button :disabled="!draft.shortcut" @click="draft.shortcut = ''; shortcutHint = ''">清除</el-button>
          </div>
        </el-form-item>
        <div class="gamma-actions">
          <el-button type="primary" native-type="submit" :disabled="busy || !draft.name.trim() || !draft.monitorId">保存</el-button>
          <el-button v-if="editingSaved" :disabled="busy" @click="request({ type: 'apply', id: draft.id })">应用已保存配置</el-button>
          <el-button v-if="editingSaved" :disabled="busy" text type="danger" @click="remove">删除</el-button>
        </div>
      </el-form>
    </div>
    <template #footer>
      <el-button :disabled="busy" @click="request({ type: 'reset' })">恢复原始 Gamma</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.gamma-note { margin: 0 0 14px; color: var(--tarkov-text-muted); }
.gamma-layout { display: grid; grid-template-columns: minmax(135px, 1fr) minmax(0, 2fr); gap: 16px; margin-top: 12px; }
.gamma-editor { max-height: 58vh; overflow: auto; padding: 0 8px; }
.profiles { display: grid; align-content: start; gap: 6px; max-height: 330px; overflow: auto; }
.profile {
  display: grid;
  gap: 3px;
  padding: 7px;
  overflow-wrap: anywhere;
  text-align: left;
  background: var(--tarkov-bg-control);
  color: var(--tarkov-text-primary);
  border: 1px solid var(--tarkov-border);
  border-radius: var(--tarkov-radius);
  cursor: pointer;
}
.profile[aria-pressed="true"] { border-color: var(--tarkov-gold); }
.profile small { color: var(--tarkov-text-muted); }
.shortcut { display: flex; width: 100%; gap: 6px; }
.shortcut-input {
  min-width: 0;
  flex: 1;
  padding: 0 9px;
  background: var(--tarkov-bg-control);
  color: var(--tarkov-text-primary);
  border: 1px solid var(--tarkov-border);
  border-radius: var(--tarkov-radius);
}
.gamma-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.gamma-actions :deep(.el-button) { margin: 0; }
</style>
