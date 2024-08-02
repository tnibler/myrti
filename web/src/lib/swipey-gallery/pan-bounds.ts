import type { Point, Size } from './util_types';

export type PanBounds = {
  readonly max: Point;
  readonly min: Point;
  readonly center: Point;
};

export function computePanBounds(slideSize: Size, panAreaSize: Size, zoomLevel: number): PanBounds {
  const x = computeBounds('width', slideSize, panAreaSize, zoomLevel);
  const y = computeBounds('height', slideSize, panAreaSize, zoomLevel);
  return {
    max: { x: x.max, y: y.max },
    min: { x: x.min, y: y.min },
    center: { x: x.center, y: y.center },
  };
}

export function clampPanToBounds(pan: Point, bounds: PanBounds): Point {
  return {
    x: Math.min(Math.max(bounds.max.x, pan.x), bounds.min.x),
    y: Math.min(Math.max(bounds.max.y, pan.y), bounds.min.y),
  };
}

function computeBounds(
  axis: 'width' | 'height',
  slideSize: Size,
  panAreaSize: Size,
  zoomLevel: number,
): { center: number; max: number; min: number } {
  const padding = 0;
  const ps = panAreaSize[axis];
  const s = slideSize[axis] * zoomLevel;
  const center = Math.round(-s / 2) + padding;
  const max = s > ps ? Math.round(ps / 2 - s) + padding : center;
  const min = s > ps ? Math.round(-ps / 2) + padding : center;
  return { center, max, min };
}
