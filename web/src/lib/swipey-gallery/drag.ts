import type { DragState } from "./gestures";
import type { GalleryControls, PagerState } from "./Pager.svelte";
import type { SlideState, SlideControls } from './Slide.svelte';
import type { Point } from "./util_types";
import { correctZoomPan } from "./zoom";

export type DragUpdate = {
  pagerMove: true,
  delta: number
} | {
  pagerMove: false,
  slidePanDelta: Point,
  /** how far the slide is dragged down relative to 1/3 of the viewport height */
  verticalDragRatio: number | undefined,
}

const PAN_END_FRICTION = 0.35;
const MIN_NEXT_SLIDE_VELOCITY = 0.5

export function updateDrag(drag: DragState, slide: SlideState, pager: PagerState): DragUpdate | null {
  const p1 = drag.p1;
  if (p1.x === p1.prev.x && p1.y === p1.prev.y) {
    return null;
  }
  const allowPanToNext = true;
  const closeOnVerticalDrag = true;
  let isMultitouch = false; // TODO not sure this is needed, whether we're ever in a drag state while there are multiple points touching
  if (closeOnVerticalDrag && drag.dragAxis === 'y'
    && slide.currentZoomLevel <= slide.zoomLevels.fit
    && !isMultitouch) {
    const deltaY = pager.isShifted ? 0 : (p1.y - p1.prev.y);
    const clampedDeltaY = Math.min(Math.max(slide.pan.y + deltaY, slide.panBounds.max.y), slide.panBounds.min.y) - slide.pan.y;
    const deltaYWithFriction = (deltaY === clampedDeltaY) ? deltaY : (deltaY * PAN_END_FRICTION);
    const verticalDragRatio = computeVerticalDragRatio(slide.pan.y, slide, pager.viewportSize.height);
    return {
      pagerMove: false,
      slidePanDelta: {
        x: 0,
        y: deltaYWithFriction
      },
      verticalDragRatio
    }
  } else {
    const deltaX = p1.x - p1.prev.x;
    const newPanX = slide.pan.x + deltaX;
    if (!slide.canBePanned && !isMultitouch) {
      return {
        pagerMove: true,
        delta: deltaX
      }
    }
    if (allowPanToNext && drag.dragAxis === 'x' && !isMultitouch) {
      if (newPanX > slide.panBounds.min.x && deltaX < 0) {
        // left to right pan
        const wasAtMinPanPosition = slide.panBounds.min.x <= drag.p1.start.x;
        if (wasAtMinPanPosition) {
          return {
            pagerMove: true,
            delta: deltaX
          }
        }
      } else if (newPanX < slide.panBounds.max.x && deltaX > 0) {
        // right to left pan
        const wasAtMaxPanPosition = drag.p1.start.x <= slide.panBounds.max.x;
        if (wasAtMaxPanPosition) {
          return {
            pagerMove: true,
            delta: deltaX
          }
        } else {
          // unsure about this
          return {
            pagerMove: false,
            slidePanDelta: {
              x: deltaX,
              y: 0
            }
          }
        }
      }
      // TODO handle if pager is shifted, drag-handler.js:283
    }
    const deltaY = pager.isShifted ? 0 : (p1.y - p1.prev.y);
    const clampedDeltaX = Math.min(Math.max(slide.pan.x + deltaX, slide.panBounds.max.x), slide.panBounds.min.x) - slide.pan.x;
    const clampedDeltaY = Math.min(Math.max(slide.pan.y + deltaY, slide.panBounds.max.y), slide.panBounds.min.y) - slide.pan.y;
    const deltaXWithFriction = (deltaX === clampedDeltaX) ? deltaX : (deltaX * PAN_END_FRICTION);
    const deltaYWithFriction = (deltaY === clampedDeltaY) ? deltaY : (deltaY * PAN_END_FRICTION);
    return {
      pagerMove: false,
      slidePanDelta: {
        x: deltaXWithFriction,
        y: deltaYWithFriction
      }
    }
  }
}

export function finishDrag(drag: DragState, gallery: GalleryControls) {
  const pager = gallery.pager;
  const slide = gallery.currentSlide;
  gallery.animations.stopAllAnimations();
  if (pager.isShifted && drag.dragVelocity) {
    const shift = pager.x - pager.currentSlideX;
    const currentSlideInView = shift / pager.viewportSize.width;
    if ((drag.dragVelocity.x < -MIN_NEXT_SLIDE_VELOCITY && currentSlideInView < 0)
      || (drag.dragVelocity.x < 0.1 && currentSlideInView < -0.5)) {
      drag.dragVelocity.x = Math.min(drag.dragVelocity.x, 0)
      gallery.pager.moveSlideAnimate('right')
      return;
    } else if ((drag.dragVelocity.x > MIN_NEXT_SLIDE_VELOCITY && currentSlideInView > 0)
      || (drag.dragVelocity.x > -0.1 && currentSlideInView > 0.5)) {
      drag.dragVelocity.x = Math.max(drag.dragVelocity.x, 0)
      gallery.pager.moveSlideAnimate('left')
      return;
    } else {
      gallery.pager.moveSlideAnimate('backToCenter')
    }
  }
  if (slide !== null &&
    (slide.currentZoomLevel > slide.zoomLevels.max
      || slide.currentZoomLevel < slide.zoomLevels.min)) {
    // Correct zoom level by bouncing back to center.
    // When fiddling with this be sure to test the following still works:
    //  - zoom in beyond the max
    //  - let one finger go
    //  - drag around
    //  - let second finger go
    // The bounce back animation should be centered on the last finger.
    // Ideally it would actually be centered on the center point between both fingers,
    // ignoring the potential very short drag between letting the first and second finger go.
    // But that's minor and not too bad really.
    const zoomPoint = { x: drag.p1.x, y: drag.p1.y }
    correctZoomPan(zoomPoint, zoomPoint, slide, gallery)
  } else {
    const slide = gallery.currentSlide;
    console.assert(slide !== null);
    if (slide === null) {
      return
    }
    finishXPan(drag, slide, gallery);
    finishYPan(drag, slide, gallery);
  }
}

function finishYPan(drag: DragState, slide: SlideControls, gallery: GalleryControls) {
  const decelerationRate = 0.995;
  const velocity = drag.dragVelocity === null ? 0 : drag.dragVelocity.y;
  const pan = slide.pan.y;
  const projectedPan = pan + project(velocity, decelerationRate);
  const clampedPan = clamp(projectedPan, slide.panBounds.max.y, slide.panBounds.min.y)

  if (slide.currentZoomLevel <= slide.zoomLevels.fit) {
    const viewportHeight = gallery.pager.viewportSize.height;
    const panRatio = computeVerticalDragRatio(slide.pan.y, slide, viewportHeight);
    const projectedPanRatio = computeVerticalDragRatio(projectedPan, slide, viewportHeight);
    if (panRatio > 0 && projectedPanRatio > 0.4) {
      gallery.close()
      return;
    }
  }

  if (pan === clampedPan) {
    // nothing to do
    return;
  }

  const dampingRatio = (clampedPan === projectedPan) ? 1 : 0.82;
  gallery.animations.startSpringAnimation({
    start: pan,
    end: clampedPan,
    velocity,
    dampingRatio,
    onUpdate: (position: number) => {
      slide.pan.y = Math.floor(position)
      gallery.onVerticalDrag(computeVerticalDragRatio(position, slide, gallery.pager.viewportSize.height));
    }
  }, 'pan');
}

function finishXPan(drag: DragState, slide: SlideControls, gallery: GalleryControls) {
  const decelerationRate = 0.995;
  const velocity = drag.dragVelocity === null ? 0 : drag.dragVelocity.x;
  const pan = slide.pan.x;
  const projectedPan = pan + project(velocity, decelerationRate);
  const clampedPan = clamp(projectedPan, slide.panBounds.max.x, slide.panBounds.min.x)
  if (pan === clampedPan) {
    // nothing to do
    return;
  }
  const dampingRatio = (clampedPan === projectedPan) ? 1 : 0.82;
  gallery.animations.startSpringAnimation({
    start: pan,
    end: clampedPan,
    velocity,
    dampingRatio,
    onUpdate: (position: number) => {
      slide.pan.x = Math.floor(position)
    }
  }, 'pan');
}

function computeVerticalDragRatio(panY: number, slide: SlideState, viewportHeight: number): number {
  const centerY = slide.panBounds.center.y;
  return 3 * (panY - centerY) / viewportHeight;
}

function project(velocity: number, decelerationRate: number): number {
  return velocity * decelerationRate / (1 - decelerationRate);
}

function clamp(n: number, min: number, max: number): number {
  return Math.min(Math.max(n, min), max);
}
