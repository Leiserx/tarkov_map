import { onMounted, onUnmounted } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  MapDetectedPayload,
  ScreenshotCoord,
  ServerConnectionStatus,
  ServerRoomSummary,
  TeamRoomState,
} from '@/types';

interface TauriEventHandlers {
  onScreenshotCoord: (payload: ScreenshotCoord) => void;
  onMapDetected: (payload: MapDetectedPayload) => void;
  onWatcherError: (payload: { target: string; message: string }) => void;
  onTeamStateUpdated: (payload: TeamRoomState) => void;
  onTeamRoomClosed: (reason: string) => void;
  onServerConnectionChanged: (payload: ServerConnectionStatus) => void;
  onServerRoomsUpdated: (payload: ServerRoomSummary[]) => void;
  onServerError: (message: string) => void;
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

    unlisteners.push(
      await listen<TeamRoomState>('team-state-updated', (event) => {
        handlers.onTeamStateUpdated(event.payload);
      })
    );

    unlisteners.push(
      await listen<string>('team-room-closed', (event) => {
        handlers.onTeamRoomClosed(event.payload);
      })
    );

    unlisteners.push(
      await listen<ServerConnectionStatus>('server-connection-changed', (event) => {
        handlers.onServerConnectionChanged(event.payload);
      })
    );

    unlisteners.push(
      await listen<ServerRoomSummary[]>('server-rooms-updated', (event) => {
        handlers.onServerRoomsUpdated(event.payload);
      })
    );

    unlisteners.push(
      await listen<string>('server-error', (event) => {
        handlers.onServerError(event.payload);
      })
    );
  });

  onUnmounted(() => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  });
}
