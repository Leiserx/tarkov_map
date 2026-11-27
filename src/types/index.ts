export interface MapBounds {
  /** [[xRight, zBottom], [xLeft, zTop]] 来自 tarkov.dev 元数据 */
  bounds: [[number, number], [number, number]];
  coordinateRotation: number;
}

export interface MapAffineCalibration {
  originX: number;
  originY: number;
  xFromX: number;
  xFromZ: number;
  yFromX: number;
  yFromZ: number;
  headingOffset: number;
}

export interface MapLayerConfig {
  id: string;
  label: string;
}

export interface MapLabelItem {
  text: string;
  position: number[];
  layerId: string;
  rotation?: number;
  size?: number;
}

export interface MapExtractItem {
  name: string;
  faction: string;
  position: number[];
  layerId: string;
}

export interface MapTransitItem {
  description: string;
  position: number[];
  layerId: string;
}

export interface MapPointsData {
  extracts: MapExtractItem[];
  transits: MapTransitItem[];
}

export type TarkovMapConfig = {
  id: string;
  nameZh: string;
  nameEn: string;
  svgPath: string;
  defaultLayer: string;
  layers: MapLayerConfig[];
} & (MapBounds | { affine: MapAffineCalibration });

export interface ScreenshotCoord {
  x: number;
  y: number;
  z: number;
  qx: number;
  qy: number;
  qz: number;
  qw: number;
  filename: string;
  timestamp: number;
}

export interface SvgPosition {
  svgX: number;
  svgY: number;
  headingDeg: number;
}

export interface AppSettings {
  playerName: string;
  lastServerAddr: string;
  lanShareEnabled: boolean;
  lanSharePort: number;
  screenshotDir: string;
  logsDir: string;
  autoDeleteScreenshot: boolean;
  maxScreenshotsKeep: number;
  autoSwitchMapByLog: boolean;
  autoCenterOnCoord: boolean;
  lastSelectedMapId: string;
}

export interface WatcherStatus {
  screenshotWatching: boolean;
  screenshotError?: string;
  logWatching: boolean;
  logError?: string;
}

export interface DetectedPaths {
  screenshotDir?: string;
  logsDir?: string;
}

export interface MapDetectedPayload {
  mapId: string;
  rawLocation: string;
}

export interface TeamMemberInfo {
  peerId: string;
  playerName: string;
  mapId: string;
  x: number | null;
  y: number | null;
  z: number | null;
  updatedAt: number;
}

export interface TeamRoomState {
  inRoom: boolean;
  isHost: boolean;
  mode: string;
  selfPeerId: string;
  roomCode: string;
  fullCode: string;
  memberCount: number;
  members: TeamMemberInfo[];
}

export interface ServerRoomSummary {
  roomId: string;
  roomName: string;
  hostName: string;
  memberCount: number;
}

export interface ServerConnectionStatus {
  connected: boolean;
  addr: string;
}

export interface LanServerStatus {
  running: boolean;
  url: string;
  error?: string;
}

export interface LanSnapshot {
  mapId: string;
  playerName: string;
  latestCoord: ScreenshotCoord | null;
  teammates: TeamMemberInfo[];
}
