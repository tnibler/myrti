import type { PanBounds } from './pan-bounds.ts';
import type { ZoomLevels } from './zoom.ts';
import type { Point, Size } from './util_types.ts';

export type SlideState = {
  readonly canBePanned: boolean;
  readonly pan: Point;
  readonly panBounds: PanBounds;
  readonly currentZoomLevel: number;
  readonly zoomLevels: ZoomLevels;
  readonly canBeZoomed: boolean;
  readonly isAtMinZoom: boolean;
  readonly isAtMaxZoom: boolean;
  readonly size: Size;
};

export type SlideControls = SlideState & {
  pan: Point;
  applyCurrentZoomPan: () => void;
  setZoomLevel: (z: number) => void;
  toggleZoom: (p: Point) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  closeTransition: (toBounds: ThumbnailBounds, onTransitionEnd: () => void) => void;
  onGrabbingStateChange: (isDragging: boolean) => void;
};

export type ThumbnailBounds = {
  /** Bounds of the DOMRect that the open animation should start from */
  rect: { x: number; y: number; width: number; height: number };
  /** Rect in the image that the thumbnail shows */
  crop?: { x: number; y: number; width: number; height: number };
};
