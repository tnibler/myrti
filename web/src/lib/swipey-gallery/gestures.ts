import { finishDrag, updateDrag } from './drag';
import type { GalleryControls } from './Pager.svelte';
import { distance, type Point } from './util_types';
import { finishZoom, updateZoom } from './zoom';

export type GestureController = {
  onClick: (e: MouseEvent) => void;
  onPointerDown: (e: PointerEvent) => void;
  onPointerUp: (e: PointerEvent) => void;
  onPointerMove: (e: PointerEvent) => void;
};

const AXIS_DRAG_HYSTERESIS = 10;
const DOUBLETAP_DELAY = 300;
const DOUBLETAP_MAX_DISTANCE = 25;

export type TrackedPoint = Point & {
  prev: Point;
  start: Point;
  // PointerEvent/TouchEvent gives us an id to keep track of which event belongs to which pointer
  id: number;
};

type ZeroPoints = {
  points: 0;
};

type OnePoint = {
  points: 1;
  p1: TrackedPoint;
};

type TwoPoints = {
  points: 2;
  p1: TrackedPoint;
  p2: TrackedPoint;
};

export type DragState = OnePoint & {
  gesture: 'drag';
  dragVelocity: Point | null;
  lastVelocityCalcTime: number;
  lastVelocityCalcP1: Point | null;
  dragAxis: 'x' | 'y' | null;
};

/** State kept in between pointer updates while a zoom gesture is happening.
 * doZoom=false means no zoom happening because current slice can't be zoomed */
type MaybeZoomState =
  | {
      doZoom: true;
      /** Pan position when zoom started */
      zoomStartPan: Point;
      startZoomLevel: number;
    }
  | { doZoom: false };

export type ZoomState = TwoPoints & MaybeZoomState & { gesture: 'zoom' };

type GestureState = DragState | ZoomState | ZeroPoints | OnePoint | TwoPoints;

export function newGestureController(
  gallery: GalleryControls,
  onMouseDetected: () => void,
): GestureController {
  let rafCallback: number | null = null;
  let state: GestureState = {
    points: 0,
  };
  let tapState: { tapTimer: number; lastTapPoint: Point } | null = null;

  function setStartPoints() {
    if (state.points === 1 || state.points === 2) {
      state.p1.start = { x: state.p1.x, y: state.p1.y };
    }
    if (state.points === 2) {
      state.p2.start = { x: state.p2.x, y: state.p2.y };
    }
    setPrevPoints();
  }

  function setPrevPoints() {
    if (state.points === 1 || state.points === 2) {
      state.p1.prev = { x: state.p1.x, y: state.p1.y };
    }
    if (state.points === 2) {
      state.p2.prev = { x: state.p2.x, y: state.p2.y };
    }
  }

  function onPointerDown(e: PointerEvent) {
    // Desktop Safari allows to drag images when preventDefault isn't called on mousedown,
    // even though preventDefault IS called on mousemove. That's why we preventDefault mousedown.
    if (e.pointerType === 'mouse') {
      onMouseDetected();
      e.preventDefault();
      // ignore if it's not left mouse button
      if (e.button > 0) {
        return;
      }
    }
    if (e.type === 'mousedown') {
      onMouseDetected();
      // TODO gestures.js:194 prevent image dragging default
    }
    gallery.animations.stopAllAnimations();

    if (state.points === 0) {
      const x = e.pageX;
      const y = e.pageY;
      const p1 = { x, y, start: { x, y }, prev: { x, y }, id: e.pointerId };
      state = {
        points: 1,
        p1,
      };
      gallery.currentSlide?.onGrabbingStateChange(true);
    } else if (state.points === 1 && !gallery.pager.isShifted) {
      // only start zooming if not currently scrolling
      const x = e.pageX;
      const y = e.pageY;
      const p2 = { x, y, start: { x, y }, prev: { x, y }, id: e.pointerId };
      setStartPoints();
      setPrevPoints();
      state = {
        points: 2,
        p1: state.p1,
        p2: p2,
      };
      gallery.currentSlide?.onGrabbingStateChange(false);
    } else if (state.points === 2) {
      gallery.currentSlide?.onGrabbingStateChange(false);
      // do nothing for now
      clearTapState();
    }
  }

  function onClick(e: MouseEvent) {
    if (gallery.pager.isShifted) {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  function onPointerUp(e: PointerEvent) {
    // console.assert(state.points > 0); // not really true, this fires on up events for the entire window
    e.preventDefault();
    if (state.points === 1) {
      rafLoopStop();
      if ('gesture' in state && state.gesture === 'drag') {
        updateDragVelocity(state, true);
        gallery.animations.stopAllAnimations();
        finishDrag(state, gallery);
      } else {
        onTap(state.p1, e);
      }
      state = {
        points: 0,
      };
      gallery.currentSlide?.onGrabbingStateChange(false);
    } else if (state.points === 2) {
      if ('gesture' in state && state.gesture === 'zoom') {
        finishZoom(state, gallery.currentSlide, gallery);
      }
      if (state.p1.id === e.pointerId) {
        // shift p2 to become p1
        state = {
          points: 1,
          p1: state.p2,
        };
      } else if (state.p2.id === e.pointerId) {
        state = {
          points: 1,
          p1: state.p1,
        };
      } else {
        console.assert(false, 'unknown pointerId in onPointerUp');
      }
      setStartPoints();
    }
  }

  function onPointerMove(e: PointerEvent) {
    e.preventDefault();
    if (e.pointerType === 'mouse') {
      onMouseDetected();
    }
    if ((state.points === 1 || state.points === 2) && state.p1.id === e.pointerId) {
      state.p1.x = e.pageX;
      state.p1.y = e.pageY;
    }
    if (state.points === 2 && state.p2.id === e.pointerId) {
      state.p2.x = e.pageX;
      state.p2.y = e.pageY;
    }

    if (state.points === 1 && !('gesture' in state)) {
      const pagerIsShifted = gallery.pager.isShifted;
      const p1 = state.p1;
      const dragAxis = computeDragAxis(p1, pagerIsShifted);
      // no gesture being tracked until now, so set start points
      // for the zoom starting now
      if (dragAxis) {
        setStartPoints();
        state = {
          points: 1,
          p1: state.p1,
          gesture: 'drag',
          dragVelocity: null,
          lastVelocityCalcTime: 0,
          lastVelocityCalcP1: { x: p1.x, y: p1.y },
          dragAxis,
        };
        gallery.animations.stopAllAnimations();
        clearTapState();
        rafLoopStop();
        rafRenderLoop();
      }
    } else if (state.points === 2 && !('gesture' in state)) {
      const currentSlide = gallery.currentSlide;
      // no gesture being tracked until now, so set start points
      // for the zoom starting now
      setStartPoints();
      setPrevPoints();
      if (currentSlide !== null && currentSlide.canBeZoomed) {
        // start zoom
        state = {
          gesture: 'zoom',
          startZoomLevel: currentSlide.currentZoomLevel,
          zoomStartPan: currentSlide.pan,
          doZoom: true,
          ...state,
        };
      } else {
        state = {
          gesture: 'zoom',
          doZoom: false,
          ...state,
        };
      }
      rafLoopStop();
      rafRenderLoop();
    }
  }

  function onTap(p: Point, e: PointerEvent) {
    if (gallery.pager.isShifted) {
      gallery.pager.moveSlideAnimate('backToCenter');
      return;
    }
    if (e.type === 'pointercancel') {
      return;
    }
    if (e.pointerType === 'mouse') {
      e.preventDefault();
      gallery.currentSlide?.toggleZoom({ x: e.x, y: e.y });
      return;
    }
    if (tapState !== null) {
      // this is the second tap within DOUBLETAP_DELAY ms,
      // if they are spatially close enough, trigger double tap
      if (distance(p, tapState.lastTapPoint) < DOUBLETAP_MAX_DISTANCE) {
        e.preventDefault();
        gallery.currentSlide?.toggleZoom({ x: e.x, y: e.y });
      }
      clearTapState();
    } else {
      tapState = {
        lastTapPoint: { x: p.x, y: p.y },
        tapTimer: setTimeout(() => {
          console.log('tap');
          // TODO signal tap
          clearTapState();
        }, DOUBLETAP_DELAY),
      };
    }
  }

  function clearTapState() {
    if (tapState !== null) {
      clearTimeout(tapState.tapTimer);
      tapState = null;
    }
  }

  function rafLoopStop() {
    if (rafCallback !== null) {
      cancelAnimationFrame(rafCallback);
      rafCallback = null;
    }
  }

  function rafRenderLoop() {
    const slideControls = gallery.currentSlide;
    if ('gesture' in state && state.gesture === 'drag') {
      updateDragVelocity(state, false);
      const pager = gallery.pager;
      const dragUpdate = updateDrag(state, slideControls, gallery.pager);
      if (dragUpdate != null) {
        if (dragUpdate.pagerMove) {
          // apply pager move
          pager.moveXBy(dragUpdate.delta);
        } else {
          // apply slide pan
          const p = slideControls.pan;
          slideControls.pan = {
            x: p.x + dragUpdate.slidePanDelta.x,
            y: p.y + dragUpdate.slidePanDelta.y,
          };
          if (dragUpdate.verticalDragRatio) {
            gallery.onVerticalDrag(dragUpdate.verticalDragRatio);
          }
        }
      }
    } else if ('gesture' in state && state.gesture === 'zoom') {
      const zoomUpdate = updateZoom(state, slideControls);
      if (zoomUpdate !== null) {
        slideControls.pan = zoomUpdate.newSlidePan;
        slideControls.setZoomLevel(zoomUpdate.newZoomLevel);
      }
    }
    setPrevPoints();
    rafCallback = requestAnimationFrame(rafRenderLoop);
  }

  return {
    onClick,
    onPointerDown,
    onPointerUp,
    onPointerMove,
  };
}

function updateDragVelocity(drag: DragState, force: boolean) {
  const p1 = drag.p1;
  if (drag.lastVelocityCalcP1 === null) {
    drag.lastVelocityCalcP1 = { x: p1.x, y: p1.y };
    return;
  }

  const now = Date.now();
  const timeSinceLastUpdate = now - drag.lastVelocityCalcTime;
  if (force || timeSinceLastUpdate >= 50) {
    const calcVelocity = (prev: number, curr: number, deltaT: number) => {
      const d = curr - prev;
      if (Math.abs(d) > 1 && deltaT > 5) {
        return d / deltaT;
      }
      return 0;
    };
    drag.dragVelocity = {
      x: calcVelocity(drag.lastVelocityCalcP1.x, p1.x, timeSinceLastUpdate),
      y: calcVelocity(drag.lastVelocityCalcP1.y, p1.y, timeSinceLastUpdate),
    };
    drag.lastVelocityCalcTime = now;
    drag.lastVelocityCalcP1 = { x: p1.x, y: p1.y };
  }
}

/** Computes whether the drag from p1.start to p1 is considered to be along an axis
 * (x or y) or not at all (null) */
function computeDragAxis(p1: TrackedPoint, pagerIsShifted: boolean): 'x' | 'y' | null {
  if (pagerIsShifted) {
    return 'x';
  }
  // calculate delta of the last touchmove tick
  const diff = Math.abs(p1.x - p1.start.x) - Math.abs(p1.y - p1.start.y);

  if (diff !== 0) {
    const majorAxis = diff > 0 ? 'x' : 'y';

    if (Math.abs(p1[majorAxis] - p1.start[majorAxis]) >= AXIS_DRAG_HYSTERESIS) {
      return majorAxis;
    }
  }
  // there is no drag axis, both axes changed equally
  // or the major axis delta was less than the minimum amount
  return null;
}
