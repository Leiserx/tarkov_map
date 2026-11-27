import type { TarkovMapConfig } from '@/types';

/**
 * SVG 地图配置，已校准地图的世界坐标边界源自 tarkov.dev。
 */
export const TARKOV_MAPS: TarkovMapConfig[] = [
  {
    id: 'customs',
    nameZh: '海关',
    nameEn: 'Customs',
    svgPath: '/maps/Customs.svg',
    coordinateRotation: 180,
    bounds: [
      [698, -307],
      [-372, 237],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层' },
      { id: 'Underground_Level', label: '地下' },
      { id: 'First_Floor', label: '一楼室内' },
      { id: 'Second_Floor', label: '二楼' },
      { id: 'Third_Floor', label: '三楼' },
    ],
  },
  {
    id: 'woods',
    nameZh: '森林',
    nameEn: 'Woods',
    svgPath: '/maps/Woods.svg',
    coordinateRotation: 180,
    bounds: [
      [646, -914],
      [-761, 442],
    ],
    defaultLayer: 'Ground_Level',
    layers: [{ id: 'Ground_Level', label: '地面层' }],
  },
  {
    id: 'interchange',
    nameZh: '立交桥',
    nameEn: 'Interchange',
    svgPath: '/maps/Interchange.svg',
    coordinateRotation: 180,
    bounds: [
      [598, -442],
      [-433, 426],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层 / 车库' },
      { id: 'First_Floor', label: '商场 1F' },
      { id: 'Second_Floor', label: '商场 2F' },
    ],
  },
  {
    id: 'reserve',
    nameZh: '储备站',
    nameEn: 'Reserve',
    svgPath: '/maps/Reserve.svg',
    coordinateRotation: 180,
    bounds: [
      [289, -274],
      [-303, 272],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层' },
      { id: 'Bunkers', label: '地下掩体' },
    ],
  },
  {
    id: 'shoreline',
    nameZh: '海岸线',
    nameEn: 'Shoreline',
    svgPath: '/maps/Shoreline.svg',
    coordinateRotation: 180,
    bounds: [
      [504, -415],
      [-1056, 618],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层' },
      { id: 'Underground_Level', label: '地下' },
      { id: 'First_Floor', label: '一楼室内' },
      { id: 'Second_Floor', label: '二楼' },
      { id: 'Third_Floor', label: '三楼' },
    ],
  },
  {
    id: 'streets-of-tarkov',
    nameZh: '塔科夫街区',
    nameEn: 'Streets of Tarkov',
    svgPath: '/maps/StreetsOfTarkov.svg',
    coordinateRotation: 180,
    bounds: [
      [323, -295],
      [-280, 532],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层' },
      { id: 'Underground_Level', label: '地下' },
      { id: 'First_Floor', label: '一楼室内' },
      { id: 'Second_Floor', label: '二楼' },
      { id: 'Third_Floor', label: '三楼' },
      { id: 'Fourth_Floor', label: '四楼' },
      { id: 'Fifth_Floor', label: '五楼' },
    ],
  },
  {
    id: 'lighthouse',
    nameZh: '灯塔',
    nameEn: 'Lighthouse',
    svgPath: '/maps/Lighthouse.svg',
    coordinateRotation: 180,
    bounds: [
      [515, -998],
      [-545, 725],
    ],
    defaultLayer: 'Ground_Level',
    layers: [{ id: 'Ground_Level', label: '地面层' }],
  },
  {
    id: 'ground-zero',
    nameZh: '中心区',
    nameEn: 'Ground Zero',
    svgPath: '/maps/GroundZero.svg',
    coordinateRotation: 180,
    bounds: [
      [249, -124],
      [-99, 364],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层' },
      { id: 'Underground_Level', label: '地下车库' },
      { id: 'First_Floor', label: '一楼室内' },
      { id: 'Second_Floor', label: '二楼' },
      { id: 'Third_Floor', label: '三楼' },
    ],
  },
  {
    id: 'factory',
    nameZh: '工厂',
    nameEn: 'Factory',
    svgPath: '/maps/Factory.svg',
    coordinateRotation: 90,
    bounds: [
      [77, -64.5],
      [-65.5, 67.4],
    ],
    defaultLayer: 'Ground_Floor',
    layers: [
      { id: 'Ground_Floor', label: '地面层 (1F)' },
      { id: 'Basement', label: '地下通道' },
      { id: 'Second_Floor', label: '二楼 (2F)' },
      { id: 'Third_Floor', label: '三楼 (3F)' },
    ],
  },
  {
    id: 'the-lab',
    nameZh: '实验室',
    nameEn: 'The Lab',
    svgPath: '/maps/Labs.svg',
    coordinateRotation: 270,
    bounds: [
      [-80, -477],
      [-287, -193],
    ],
    defaultLayer: 'First_Level',
    layers: [
      { id: 'First_Level', label: '主层 (1F)' },
      { id: 'Technical_Level', label: '地下管道层' },
      { id: 'Second_Level', label: '二楼 (2F)' },
    ],
  },
  {
    id: 'terminal',
    nameZh: '终点站',
    nameEn: 'Terminal',
    svgPath: '/maps/Terminal.svg',
    coordinateRotation: 180,
    bounds: [
      [463, -580],
      [-433, 475],
    ],
    defaultLayer: 'Ground_Level',
    layers: [
      { id: 'Ground_Level', label: '地面层' },
      { id: 'First_Floor', label: '一楼室内' },
    ],
  },
  {
    id: 'labyrinth',
    nameZh: '迷宫',
    nameEn: 'The Labyrinth',
    svgPath: '/maps/Labyrinth.svg',
    // EFTarkov 迷宫截图定位参数，相对于 1300 × 1200 地图画布左上角。
    affine: {
      originX: 484.291797,
      originY: 601.657474,
      xFromX: -0.011235,
      xFromZ: 10.002859,
      yFromX: 9.995898,
      yFromZ: -0.00756,
      headingOffset: -90,
    },
    defaultLayer: 'main',
    layers: [{ id: 'main', label: '迷宫' }],
  },
];

export function getMapById(id: string): TarkovMapConfig {
  return TARKOV_MAPS.find((m) => m.id === id) ?? TARKOV_MAPS[0];
}
