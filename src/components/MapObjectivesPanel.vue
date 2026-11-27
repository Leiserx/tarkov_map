<script setup lang="ts">
import { computed, ref } from 'vue';
import type { MapObjectiveEntry, MapObjectiveLocation } from '@/types';

const props = defineProps<{ entries: MapObjectiveEntry[] }>();
const collapsed = defineModel<boolean>('collapsed', { required: true });
const emit = defineEmits<{
  select: [locations: MapObjectiveLocation[] | null, focus: MapObjectiveLocation[] | null];
  details: [entry: MapObjectiveEntry];
}>();

const query = ref('');
const showingAll = ref(false);
const activeTab = ref<MapObjectiveEntry['kind']>('task');
const selected = ref(new Map<string, MapObjectiveLocation[]>());
const tabs = [
  { kind: 'task', label: '任务' },
  { kind: 'story', label: '剧情' },
] as const;
const visibleEntries = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return props.entries.filter((entry) =>
    entry.kind === activeTab.value &&
    [entry.name, entry.search, ...(entry.objectives ?? []), ...entry.locations.map((location) => location.title)]
      .some((text) => text?.toLowerCase().includes(keyword))
  );
});

function toggleAll() {
  showingAll.value = !showingAll.value;
  selected.value.clear();
  emit('select', showingAll.value ? null : [], null);
}

function updateSelection(focus: MapObjectiveLocation[] | null) {
  showingAll.value = false;
  emit('select', [...selected.value.values()].flat(), focus);
}

function toggleEntry(entry: MapObjectiveEntry) {
  if (selected.value.delete(entry.id)) {
    updateSelection(null);
    return;
  }
  selected.value.set(entry.id, entry.locations);
  updateSelection(entry.locations);
}

function toggleLocation(entry: MapObjectiveEntry, location: MapObjectiveLocation) {
  const current = selected.value.get(entry.id);
  const isIsolated = entry.locations.length > 1 && current?.length === 1 && current[0].id === location.id;
  selected.value.set(entry.id, isIsolated ? entry.locations : [location]);
  updateSelection(isIsolated ? null : [location]);
}
</script>

<template>
  <aside class="objectives-panel" :class="{ collapsed }" aria-label="任务和剧情">
    <header class="panel-header">
      <strong v-if="!collapsed">任务 / 剧情</strong>
      <button
        class="toggle"
        type="button"
        :aria-expanded="!collapsed"
        :aria-label="collapsed ? '展开任务侧栏' : '收起任务侧栏'"
        @click="collapsed = !collapsed"
      >{{ collapsed ? '»' : '«' }}</button>
    </header>
    <template v-if="!collapsed">
      <div class="panel-tools">
        <el-input v-model="query" clearable size="small" placeholder="搜索任务或剧情" aria-label="搜索任务或剧情" />
        <button type="button" class="entry" :aria-pressed="showingAll" @click="toggleAll">
          {{ showingAll ? '隐藏全部' : '显示全部' }}
        </button>
      </div>
      <el-tabs v-model="activeTab" class="objective-tabs" stretch>
        <el-tab-pane v-for="tab in tabs" :key="tab.kind" :name="tab.kind" :label="tab.label">
          <div v-if="activeTab === tab.kind" class="panel-list">
            <p v-if="!visibleEntries.length" class="status">
              {{ query.trim() ? `没有匹配的${tab.label}。` : `此地图暂无${tab.label}数据。` }}
            </p>
            <div v-for="entry in visibleEntries" :key="entry.id">
              <div class="entry-row">
                <button type="button" class="entry" :aria-pressed="selected.has(entry.id)" @click="toggleEntry(entry)">
                  <span>{{ entry.name }}</span>
                  <small>{{ entry.locations.length ? `${entry.locations.length} 处` : '无固定点位' }}</small>
                </button>
                <button type="button" class="details" :aria-label="`${entry.name}详情`" @click="emit('details', entry)">详情</button>
              </div>
              <div v-if="selected.has(entry.id)" class="entry-detail">
                <ul v-if="entry.objectives?.length" class="instructions">
                  <li v-for="(objective, index) in entry.objectives" :key="index">{{ objective }}</li>
                </ul>
                <button
                  v-for="(location, index) in entry.locations"
                  :key="location.id"
                  type="button"
                  class="entry location"
                  :aria-pressed="selected.get(entry.id)?.length === 1 && selected.get(entry.id)?.[0].id === location.id"
                  @click="toggleLocation(entry, location)"
                >
                  <span>{{ index + 1 }}. {{ location.title }}</span>
                  <small>{{ location.floor === '-1' ? '地下 1 层' : `${location.floor} 层` }}{{ location.area ? ' · 区域' : '' }}</small>
                </button>
              </div>
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </template>
  </aside>
</template>

<style scoped>
.objectives-panel {
  width: clamp(200px, 24vw, 292px);
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  background: var(--tarkov-bg-panel, #191b1d);
  border-right: 1px solid #383a3e;
}

.objectives-panel.collapsed { width: 38px; }

.panel-header,
.entry {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.panel-header { padding: 8px; }
.panel-tools { display: grid; gap: 6px; padding: 0 8px 8px; }
.objective-tabs { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.objective-tabs :deep(.el-tabs__header) { margin: 0 8px 8px; }
.objective-tabs :deep(.el-tabs__content) { flex: 1; min-height: 0; }
.objective-tabs :deep(.el-tab-pane) { height: 100%; }
.panel-list { height: 100%; overflow: auto; overscroll-behavior: contain; padding: 0 8px 8px; }
.status, .instructions { font-size: 12px; color: #c4c8ce; }
.instructions { padding-left: 18px; }

.toggle,
.details,
.entry {
  background: #222529;
  border: 1px solid transparent;
  border-radius: 4px;
  color: #e7e9ed;
  cursor: pointer;
}

.toggle { padding: 1px 5px; font-size: 17px; }
.entry-row { display: flex; gap: 4px; align-items: stretch; }
.details { flex-shrink: 0; margin-bottom: 3px; padding: 0 6px; font-size: 12px; }
.entry { width: 100%; margin-bottom: 3px; padding: 6px; text-align: left; }
.entry small { color: #bac0c9; flex-shrink: 0; }
.entry:hover, .toggle:hover, .details:hover { background: #30343a; }
.entry[aria-pressed="true"] { border-color: #b39b69; background: #353026; }
.entry-detail { margin: 4px 0 8px 7px; }
.location { font-size: 12px; }
</style>
