import { onMounted, onUnmounted } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { MapDetectedPayload, ScreenshotCoord } from '@/types';

interface TauriEventHandlers {
  onScreenshotCoord: (payload: ScreenshotCoord) => void;
  onMapDetected: (payload: MapDetectedPayload) => void;
  onWatcherError: (payload: { target: string; message: string }) => void;
}

export function useTauriEvents(handlers: TauriEventHandlers) {
  const unlisteners: UnlistenFn[] = [];

  onMounted(async () => {
    unlisteners.push(
      await listen<ScreenshotCoord>('screenshot-coord', (event) => {
        handlers.onScreenshotCoord(event.payload);
      })
    );

    unlisteners.push(
      await listen<MapDetectedPayload>('map-detected', (event) => {
        handlers.onMapDetected(event.payload);
      })
    );

    unlisteners.push(
      await listen<{ target: string; message: string }>('watcher-error', (event) => {
        handlers.onWatcherError(event.payload);
      })
    );
  });

  onUnmounted(() => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  });
}
