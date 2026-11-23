<script setup lang="ts">
import { reactive, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettings, DetectedPaths } from '@/types';

const props = defineProps<{
  visible: boolean;
  settings: AppSettings;
}>();

const emit = defineEmits<{
  (e: 'update:visible', val: boolean): void;
  (e: 'save', val: AppSettings): void;
}>();

const form = reactive<AppSettings>({ ...props.settings });

watch(
  () => props.visible,
  (isOpen) => {
    if (isOpen) {
      Object.assign(form, props.settings);
    }
  }
);

async function pickDirectory(target: 'screenshotDir' | 'logsDir') {
  const selected = await open({
    directory: true,
    multiple: false,
    title: target === 'screenshotDir' ? '选择塔科夫截图目录' : '选择塔科夫 Logs 目录',
  });
  if (typeof selected === 'string') {
    form[target] = selected;
  }
}

async function autoDetect() {
  const detected = await invoke<DetectedPaths>('detect_default_paths');
  if (detected.screenshotDir) {
    form.screenshotDir = detected.screenshotDir;
  }
  if (detected.logsDir) {
    form.logsDir = detected.logsDir;
  }
}

function handleSave() {
  emit('save', { ...form });
  emit('update:visible', false);
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    title="系统设置"
    width="540px"
    @update:model-value="emit('update:visible', $event)"
  >
    <el-form label-position="top" size="default">
      <el-form-item label="玩家自定义名称（显示在定位标记旁）">
        <el-input v-model="form.playerName" maxlength="24" placeholder="请输入你的昵称" />
      </el-form-item>

      <el-form-item label="游戏截图目录（Screenshots）">
        <div class="path-row">
          <el-input v-model="form.screenshotDir" placeholder="例如 C:\Users\...\Documents\Escape from Tarkov\Screenshots" />
          <el-button @click="pickDirectory('screenshotDir')">浏览</el-button>
        </div>
      </el-form-item>

      <el-form-item label="游戏日志目录（Logs，用于自动识别进图地图）">
        <div class="path-row">
          <el-input v-model="form.logsDir" placeholder="例如 C:\Battlestate Games\Escape from Tarkov\Logs" />
          <el-button @click="pickDirectory('logsDir')">浏览</el-button>
        </div>
      </el-form-item>

      <el-form-item>
        <el-button size="small" type="primary" plain @click="autoDetect">
          自动探测默认路径
        </el-button>
      </el-form-item>

      <el-divider />

      <el-form-item label="截图文件处理">
        <div class="switch-row">
          <el-switch v-model="form.autoDeleteScreenshot" />
          <span class="switch-desc">
            开启自动清理旧截图（默认关闭，开启可节省磁盘空间）
          </span>
        </div>
      </el-form-item>

      <el-form-item v-if="form.autoDeleteScreenshot" label="允许保留最新截图数量">
        <div class="switch-row">
          <el-input-number v-model="form.maxScreenshotsKeep" :min="0" :max="500" size="small" />
          <span class="switch-desc">
            设为 0 表示定位后立即删除；大于 0 则保留最新的 N 张截图
          </span>
        </div>
      </el-form-item>

      <el-form-item label="战局地图切换">
        <div class="switch-row">
          <el-switch v-model="form.autoSwitchMapByLog" />
          <span class="switch-desc">
            监听游戏 Logs 目录并在进入战局时自动切换至对应地图
          </span>
        </div>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="emit('update:visible', false)">取消</el-button>
      <el-button type="primary" @click="handleSave">保存并应用</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.path-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.switch-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.switch-desc {
  font-size: 12px;
  color: var(--tarkov-text-muted);
}
</style>
