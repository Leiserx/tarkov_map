<script setup lang="ts">
import { Aim, Minus, Plus, RefreshLeft } from '@element-plus/icons-vue';

defineProps<{
  hasPosition: boolean;
}>();

const emit = defineEmits<{
  (e: 'zoomIn'): void;
  (e: 'zoomOut'): void;
  (e: 'centerPlayer'): void;
  (e: 'resetView'): void;
}>();
</script>

<template>
  <div class="map-controls">
    <el-tooltip content="放大" placement="left">
      <el-button circle size="small" :icon="Plus" @click="emit('zoomIn')" />
    </el-tooltip>
    <el-tooltip content="缩小" placement="left">
      <el-button circle size="small" :icon="Minus" @click="emit('zoomOut')" />
    </el-tooltip>
    <el-tooltip :content="hasPosition ? '居中到当前位置' : '等待首次截图'" placement="left">
      <el-button
        circle
        size="small"
        :icon="Aim"
        :disabled="!hasPosition"
        @click="emit('centerPlayer')"
      />
    </el-tooltip>
    <el-tooltip content="重置地图视图" placement="left">
      <el-button circle size="small" :icon="RefreshLeft" @click="emit('resetView')" />
    </el-tooltip>
  </div>
</template>

<style scoped>
.map-controls {
  position: absolute;
  right: 16px;
  bottom: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
  border-radius: 10px;
  background: rgba(24, 27, 34, 0.88);
  border: 1px solid var(--tarkov-border);
  backdrop-filter: blur(8px);
  z-index: 15;
}

.map-controls :deep(.el-button + .el-button) {
  margin-left: 0;
}
</style>
