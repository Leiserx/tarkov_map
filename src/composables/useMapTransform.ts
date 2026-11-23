import type { ScreenshotCoord, SvgPosition, TarkovMapConfig } from '@/types';

/**
 * 按地图 coordinateRotation 旋转二维向量
 */
export function rotatePoint2D(x: number, z: number, rotationDeg: number): [number, number] {
  const rad = (rotationDeg * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  return [x * cos - z * sin, x * sin + z * cos];
}

/**
 * 将 Unity 四元数 (qx, qy, qz, qw) 转换为水平面朝向向量并计算 SVG 旋转角度（0° 朝上，顺时针增加）
 */
export function computeHeadingDeg(
  coord: Pick<ScreenshotCoord, 'qx' | 'qy' | 'qz' | 'qw'>,
  coordinateRotation: number
): number {
  const { qx, qy, qz, qw } = coord;
  // Unity 本地前向向量 (0, 0, 1) 经四元数旋转后的水平分量
  const fx = 2 * (qx * qz + qw * qy);
  const fz = 1 - 2 * (qx * qx + qy * qy);

  const [rfx, rfz] = rotatePoint2D(fx, fz, coordinateRotation);
  return (Math.atan2(rfx, rfz) * 180) / Math.PI;
}

/**
 * 将游戏世界二维坐标 (worldX, worldZ) 映射为 SVG viewBox 坐标 (svgX, svgY)
 * 注意：tarkov.dev 的 bounds 为未旋转的原始世界坐标，需与点位一同应用 coordinateRotation
 */
export function worldPointToSvg(
  worldX: number,
  worldZ: number,
  mapConfig: TarkovMapConfig,
  svgWidth: number,
  svgHeight: number
): { svgX: number; svgY: number } {
  const rot = mapConfig.coordinateRotation;
  const [rx, rz] = rotatePoint2D(worldX, worldZ, rot);

  const [[x0, z0], [x1, z1]] = mapConfig.bounds;
  const [bx0, bz0] = rotatePoint2D(x0, z0, rot);
  const [bx1, bz1] = rotatePoint2D(x1, z1, rot);

  const xMin = Math.min(bx0, bx1);
  const xMax = Math.max(bx0, bx1);
  const zMin = Math.min(bz0, bz1);
  const zMax = Math.max(bz0, bz1);

  const spanX = xMax - xMin || 1;
  const spanZ = zMax - zMin || 1;

  return {
    svgX: ((rx - xMin) / spanX) * svgWidth,
    svgY: ((zMax - rz) / spanZ) * svgHeight,
  };
}

/**
 * 将游戏世界坐标 (X, Z) 映射为 SVG viewBox 坐标与朝向角度
 */
export function worldToSvgPosition(
  coord: ScreenshotCoord,
  mapConfig: TarkovMapConfig,
  svgWidth: number,
  svgHeight: number
): SvgPosition {
  const { svgX, svgY } = worldPointToSvg(coord.x, coord.z, mapConfig, svgWidth, svgHeight);
  const headingDeg = computeHeadingDeg(coord, mapConfig.coordinateRotation);

  return { svgX, svgY, headingDeg };
}
