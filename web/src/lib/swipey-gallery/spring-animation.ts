/*
The code in this file was originally written by Dmitry Semenov as part of
Photoswipe (https://github.com/dimsemenov/photoswipe), released under the
MIT License.
As this file specifically is barely a derivative of the original work,
it is licensed under the same terms (MIT License) and exempt from the
AGPLv3 terms applying to the rest of this repository.
The copyright notice of the original license is reproduced here:

The MIT License (MIT)

Copyright (c) 2014-2022 Dmitry Semenov, https://dimsemenov.com

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
*/

export type SpringParams = {
  start: number;
  end: number;
  velocity: number;
  dampingRatio: number | undefined;
  frequency: number | undefined;
  onUpdate: (position: number) => void;
  onFinish: () => void | undefined;
};

type CancelAnimation = {
  cancel: () => void;
};

const DEFAULT_DAMPING_RATIO = 0.75;
const DEFAULT_FREQUENCY = 12;

export function springAnimation(params: SpringParams): CancelAnimation {
  const { start, end, onUpdate, onFinish } = params;
  const dampingRatio = params.dampingRatio ?? DEFAULT_DAMPING_RATIO;
  const frequency = params.frequency ?? DEFAULT_FREQUENCY;

  let raf: number | null = null;
  let prevTime = Date.now();
  let deltaPosition = start - end;
  let velocity = params.velocity;

  const easer = springEaser(dampingRatio, frequency);

  const animationLoop = () => {
    const now = Date.now();
    if (raf) {
      const e = easer.ease(deltaPosition, now - prevTime, velocity);
      deltaPosition = e.dp;
      velocity = e.velocity;

      if (Math.abs(deltaPosition) < 1 && Math.abs(velocity) < 50) {
        onUpdate(end);
        onFinish();
      } else {
        prevTime = Date.now();
        onUpdate(deltaPosition + end);
        raf = requestAnimationFrame(animationLoop);
      }
    }
  };

  raf = requestAnimationFrame(animationLoop);
  return {
    cancel: () => {
      cancelAnimationFrame(raf!);
      raf = null;
    },
  };
}

type Easer = {
  ease: (position: number, deltaTime: number, velocity: number) => { dp: number; velocity: number };
};

/**
 * @param position (currentPos - end)
 * @param deltaTime milliseconds */
function springEaser(dampingRatio: number, frequency: number): Easer {
  console.assert(dampingRatio <= 1, 'invalid springAnimation params');
  const dampedFrequency =
    dampingRatio < 1 ? frequency * Math.sqrt(1 - dampingRatio ** 2) : frequency;

  const ease = (position: number, deltaTime: number, velocity: number) => {
    deltaTime /= 1000;
    const dampingPower = Math.E ** (-dampingRatio * frequency * deltaTime);

    if (dampingRatio === 1) {
      const coeff = velocity + frequency * position;

      const dp = (position + coeff * deltaTime) * dampingPower;

      const newVelocity = dp * -frequency + coeff * dampingPower;
      return { dp, velocity: newVelocity };
    } else if (dampingRatio < 1) {
      const coeff = (1 / dampedFrequency) * (dampingRatio * frequency * position + velocity);

      const dampedFCos = Math.cos(dampedFrequency * deltaTime);
      const dampedFSin = Math.sin(dampedFrequency * deltaTime);

      const dp = dampingPower * (position * dampedFCos + coeff * dampedFSin);

      const newVelocity =
        dp * -frequency * dampingRatio +
        dampingPower *
          (-dampedFrequency * position * dampedFSin + dampedFrequency * coeff * dampedFCos);
      return { dp, velocity: newVelocity };
    }
    return { dp: 0, velocity: 0 };
  };
  return { ease };
}
