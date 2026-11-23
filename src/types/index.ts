export interface MapBounds {
  /** [[xRight, zBottom], [xLeft, zTop]] 来自 tarkov.dev 元数据 */
  bounds: [[number, number], [number, number]];
  coordinateRotation: number;
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

export interface TarkovMapConfig extends MapBounds {
  id: string;
  nameZh: string;
  nameEn: string;
  svgPath: string;
  defaultLayer: string;
  layers: MapLayerConfig[];
}

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
