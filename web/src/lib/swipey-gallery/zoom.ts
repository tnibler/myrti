import type { ZoomState } from "./gestures";
import { type PanBounds, clampPanToBounds, computePanBounds } from "./pan-bounds";
import { pointsEqual, type Point, type Size, distance } from "./util_types"
import type { SlideState, SlideControls } from './Slide.svelte';
import type { GalleryControls } from './Pager.svelte';

export type ZoomLevels = {
  fit: number,
  fill: number,
  vFill: number,
  secondary: number,
  max: number,
  min: number,
}


const MAX_IMAGE_WIDTH = 4000;
const ZOOM_OUT_FRICTION = 0.15;
const ZOOM_IN_FRICTION = 0.25;

export function computeZoomLevels({ maxSize, panAreaSize }: { maxSize: Size, panAreaSize: Size }): ZoomLevels {
  const hRatio = panAreaSize.width / maxSize.width;
  const vRatio = panAreaSize.height / maxSize.height;

  const fit = Math.min(1, hRatio < vRatio ? hRatio : vRatio);
  const fill = Math.min(1, hRatio > vRatio ? hRatio : vRatio);
  const vFill = Math.min(1, vRatio);

  const fit3x = Math.min(1, fit * 3);

  const secondary = (maxSize.width * fit3x <= MAX_IMAGE_WIDTH) ? fit3x : MAX_IMAGE_WIDTH / maxSize.width;
  const max = fit * 4;
  return {
    fit, fill, vFill, secondary, max, min: fit
  }
}

export type ZoomUpdate = {
  newSlidePan: Point,
  newZoomLevel: number
};

export function updateZoom(state: ZoomState, slide: SlideState): ZoomUpdate | null {
  if (!state.doZoom
    || (pointsEqual(state.p1, state.p1.prev) && pointsEqual(state.p2, state.p2.prev))) {
    return null;
  }
  const p1 = state.p1;
  const p2 = state.p2;
  const minZoomLevel = slide.zoomLevels.min;
  const maxZoomLevel = slide.zoomLevels.max;
  const zoomPoint = centerPoint(p1, p2);
  const zoomStartPoint = centerPoint(p1.start, p2.start);
  // zoom level without any correction/clamping/friction
  const rawZoomLevel = state.startZoomLevel * distance(p1, p2) / distance(p1.start, p2.start);
  let zoomLevel = rawZoomLevel;
  if (rawZoomLevel < minZoomLevel) {
    // todo bgOpacity, pinch to close
    zoomLevel = minZoomLevel - (minZoomLevel - rawZoomLevel) * ZOOM_OUT_FRICTION;
  } else if (rawZoomLevel > maxZoomLevel) {
    zoomLevel = maxZoomLevel + (rawZoomLevel - maxZoomLevel) * ZOOM_IN_FRICTION;
  }
  const newSlidePan = {
    x: computePan('x', zoomLevel, state.startZoomLevel, zoomPoint, zoomStartPoint, state.zoomStartPan),
    y: computePan('y', zoomLevel, state.startZoomLevel, zoomPoint, zoomStartPoint, state.zoomStartPan),
  }
  return {
    newSlidePan,
    newZoomLevel: zoomLevel
  };
}

export function finishZoom(state: ZoomState, slide: SlideControls, gallery: GalleryControls) {
  if (!state.doZoom) {
    return;
  }
  const p1 = state.p1;
  const p2 = state.p2;
  const rawZoomLevel = state.startZoomLevel * distance(p1, p2) / distance(p1.start, p2.start);
  // TODO pinch to close
  // const isOverMaxZoomLevel = rawZoomLevel > (slide.zoomLevels.fit * 1.15); // needed for pinch to close
  const zoomPoint = centerPoint(p1, p2);
  const zoomStartPoint = centerPoint(p1.start, p2.start);
  correctZoomPan(zoomPoint, slide, gallery);
}

export function correctZoomPan(
  zoomPoint: Point,
  slide: SlideControls,
  gallery: GalleryControls,
) {
  if (!slide.canBeZoomed) {
    return;
  }
  const initialZoomLevel = slide.currentZoomLevel;
  const initialPan = { x: slide.pan.x, y: slide.pan.y };
  const zoomNeedsCorrected = initialZoomLevel < slide.zoomLevels.min || initialZoomLevel > slide.zoomLevels.max;
  const correctedZoomLevel = Math.max(Math.min(initialZoomLevel, slide.zoomLevels.max), slide.zoomLevels.min);
  // pan after hypothetically setting correctedZoomLevel
  const zoomAdjustedPan = {
    // zoomPoint is passed as both zoomPoint and zoomStartPoint since the bounce back animation
    // is really a new zoom gesture without any movement of the finger points
    x: computePan('x', correctedZoomLevel, initialZoomLevel, zoomPoint, zoomPoint, initialPan),
    y: computePan('y', correctedZoomLevel, initialZoomLevel, zoomPoint, zoomPoint, initialPan),
  }
  // now clamp zoomAdjustedPan to bounds after hypothetically setting correctedZoomLevel
  // panAreaSize is really always gallery.viewportSize
  const panBoundsWithCorrectedZoom: PanBounds = computePanBounds(slide.size, gallery.pager.viewportSize, correctedZoomLevel);
  const finalCorrectedPan: Point = clampPanToBounds(zoomAdjustedPan, panBoundsWithCorrectedZoom);

  const panNeedsCorrected = !pointsEqual(finalCorrectedPan, initialPan);
  if (!zoomNeedsCorrected && !panNeedsCorrected) {
    // slide.setZoomLevel(correctedZoomLevel);
    // slide.pan = finalCorrectedPan;
    slide.applyCurrentZoomPan();
    return;
  }

  const panDeltaX = finalCorrectedPan.x - initialPan.x;
  const panDeltaY = finalCorrectedPan.y - initialPan.y;
  const zoomDelta = correctedZoomLevel - initialZoomLevel;
  gallery.animations.stopAnimationsFor('pan');
  gallery.animations.startSpringAnimation({
    start: 0,
    end: 1000,
    velocity: 0,
    dampingRatio: 1,
    frequency: 40,
    onUpdate: (tt) => {
      const t = tt / 1000; // normalize from 0 to 1
      if (panNeedsCorrected) {
        const pan = {
          x: initialPan.x + panDeltaX * t,
          y: initialPan.y + panDeltaY * t
        };
        slide.pan = pan;
      }
      if (zoomNeedsCorrected) {
        const zoom = initialZoomLevel + zoomDelta * t;
        slide.setZoomLevel(zoom);
      }
    },
    onFinish: () => {
      slide.applyCurrentZoomPan();
    }
  }, 'pan');
}

function computePan(axis: 'x' | 'y', zoomLevel: number, startZoomLevel: number, zoomPoint: Point, zoomStartPoint: Point, zoomStartPan: Point): number {
  const zoomFactor = zoomLevel / startZoomLevel;
  return zoomPoint[axis] - ((zoomStartPoint[axis] - zoomStartPan[axis]) * zoomFactor);
}

function centerPoint(p1: Point, p2: Point): Point {
  return {
    x: (p1.x + p2.x) / 2,
    y: (p1.y + p2.y) / 2
  }
}
