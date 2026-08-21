// https://github.com/VectorPrivacy/fast-thumbhash
// MIT License
//
// Copyright (c) 2025 VectorPrivacy
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

const B91_ALPHABET =
  'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~ ';

const B91_DECODE = new Uint8Array(256).fill(255);
for (let i = 0; i < B91_ALPHABET.length; i++) {
  B91_DECODE[B91_ALPHABET.charCodeAt(i)] = i;
}

export function base91Decode(encoded: string): Uint8Array | null {
  const out: number[] = [];
  let n = 0;
  let bits = 0;
  let queue = -1;

  for (let i = 0; i < encoded.length; i++) {
    const byte = encoded.charCodeAt(i);
    if (byte > 255) return null; // not ASCII?

    const d = B91_DECODE[byte];
    if (d === 255) {
      // bad character
      return null;
    }

    if (queue < 0) {
      queue = d;
    } else {
      const v = queue + d * 91;
      queue = -1;

      n |= v << bits;
      bits += (v & 8191) > 88 ? 13 : 14;

      while (bits >= 8) {
        out.push(n & 255);
        n >>>= 8;
        bits -= 8;
      }
    }
  }

  if (queue >= 0) {
    out.push((n | (queue << bits)) & 255);
  }
  return new Uint8Array(out);
}
