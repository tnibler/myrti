import { springAnimation } from "./spring-animation";

export type SpringParams = {
  start: number,
  end: number,
  velocity: number,
  dampingRatio: number,
  frequency: number | undefined,
  onUpdate: (position: number) => void;
  onFinish: () => void | undefined;
}

export type AnimationPurpose = 'pan' | 'pager';

export type AnimationControls = {
  stopAllAnimations: () => void;
  stopAnimationsFor: (purpose: AnimationPurpose) => void;
  startSpringAnimation: (p: SpringParams, purpose: AnimationPurpose) => void;
}

type Animation = {
  cancel: () => void;
}

export function newAnimationControls(): AnimationControls {
  const anims: Record<AnimationPurpose, Animation[]> = {
    pager: [],
    pan: [],
  }

  const stopAllAnimations = () => {
    anims.pan.forEach((a) => a.cancel());
    anims.pan.length = 0;
    anims.pager.forEach((a) => a.cancel());
    anims.pager.length = 0;
  }

  const stopAnimationsFor = (purpose: AnimationPurpose) => {
    anims[purpose].forEach((a) => a.cancel());
    anims[purpose].length = 0;
  }

  const startSpringAnimation = (p: SpringParams, purpose: AnimationPurpose) => {
    const anim = springAnimation({
      ...p,
      onFinish: () => {
        if (p.onFinish) {
          p.onFinish();
        }
        anim.cancel();
        const idx = anims[purpose].indexOf(anim);
        if (idx >= 0) {
          anims[purpose].splice(idx);
        }
      },
    });
    anims[purpose].push(anim)
  }

  return {
    stopAllAnimations,
    stopAnimationsFor,
    startSpringAnimation,
  }
}
