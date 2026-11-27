<script setup lang="ts">
import type { MapObjectiveEntry, MapObjectiveLocation } from '@/types';

defineProps<{
  entry: MapObjectiveEntry | null;
  location: MapObjectiveLocation | null;
}>();
const emit = defineEmits<{
  close: [];
  select: [location: MapObjectiveLocation | null];
}>();
</script>

<template>
  <el-dialog
    :model-value="!!entry"
    :title="entry?.name"
    width="min(640px, 92vw)"
    @update:model-value="!$event && emit('close')"
  >
    <div v-if="entry" class="objective-detail">
      <p v-if="entry.trader" class="objective-meta">{{ entry.trader }}</p>
      <ul v-if="entry.objectives?.length" class="objective-steps">
        <li v-for="(step, index) in entry.objectives" :key="index">{{ step }}</li>
      </ul>
      <template v-if="location">
        <button type="button" class="detail-link" @click="emit('select', null)">返回地点列表</button>
        <h3>{{ location.title }}</h3>
        <div v-if="location.images?.length" class="photo-grid">
          <el-image
            v-for="(image, index) in location.images"
            :key="image"
            :src="image"
            :alt="`${location.title} · 图片 ${index + 1}`"
            :preview-src-list="location.images"
            :initial-index="index"
            fit="contain"
            preview-teleported
            hide-on-click-modal
          >
            <template #error><span class="photo-status">图片加载失败</span></template>
          </el-image>
        </div>
        <p v-else class="objective-meta">来源页面未提供此地点的图片。</p>
      </template>
      <template v-else-if="entry.locations.length">
        <h3>任务地点</h3>
        <ul class="location-list">
          <li v-for="(item, index) in entry.locations" :key="item.id">
            <button type="button" class="detail-link" @click="emit('select', item)">
              <span>{{ index + 1 }}. {{ item.title }}</span>
              <small>
                {{ item.floor === '-1' ? '地下 1 层' : `${item.floor} 层` }}
                {{ item.images?.length ? ` · ${item.images.length} 张图片` : '' }}
              </small>
            </button>
          </li>
        </ul>
      </template>
      <p v-else class="objective-meta">此任务无固定地图地点。</p>
    </div>
  </el-dialog>
</template>

<style scoped>
.objective-detail { max-height: 65vh; overflow: auto; color: var(--tarkov-text-primary); }
.objective-meta { color: var(--tarkov-text-muted); margin: 0 0 12px; }
.objective-steps { padding-left: 19px; }
.location-list { list-style: none; padding: 0; }
.location-list li + li { margin-top: 6px; }
.detail-link {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px 12px;
  text-align: left;
  border: 1px solid var(--tarkov-border);
  border-radius: var(--tarkov-radius);
  background: var(--tarkov-bg-control);
  color: inherit;
  padding: 7px 10px;
  cursor: pointer;
}
.detail-link:hover { border-color: var(--tarkov-gold); }
.detail-link small { color: var(--tarkov-text-muted); }
.photo-grid { display: grid; gap: 10px; }
.photo-grid .el-image { width: 100%; min-height: 90px; background: var(--tarkov-bg-control); }
.photo-status { display: block; padding: 16px; color: var(--tarkov-text-muted); }
</style>
