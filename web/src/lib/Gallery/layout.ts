import { dijkstra } from './dijkstra';

export function round(n) {
  return Math.round(n * 100 + Number.EPSILON) / 100;
}

export function ratio(width: number, height: number): number {
  return round(width / height);
}

export function scaleHeight(width: number, ratio: number): number {
  return round(width / ratio);
}

export function scaleWidth(height: number, ratio: number): number {
  return round(height * ratio);
}

function getRowHeight(row, containerWidth: number, gutter: number): number {
  const rowWidth = containerWidth - (row.length - 1) * gutter;
  const rowAspectRatio = row.reduce((acc, { ratio }) => acc + ratio, 0);
  return scaleHeight(rowWidth, rowAspectRatio);
}

function cost(images, start: number, end: number, containerWidth: number, targetHeight: number, gutter: number): number {
  const row = images.slice(start, end);
  const rowHeight = getRowHeight(row, containerWidth, gutter);
  return Math.pow(Math.abs(rowHeight - targetHeight), 2);
}

function calcSeekLimit(containerWidth: number, targetRowHeight: number): number {
  if (containerWidth < 420) {
    // limit to two nodes if the container is narrow
    return 2;
  }

  // find how many 3/4 portrait pictures will fit in an ideal row
  const count = ratio(containerWidth, targetRowHeight) / 0.75;
  return Math.round(count * 1.5);
}

export interface ImageSize {
  width: number,
  height: number
};

interface LayoutParams {
  images: ImageSize[],
  containerWidth: number,
  targetHeight: number,
  gutter: number,
}

export interface ImageLayout {
  scaledWidth: number,
  scaledHeight: number,
  scaledWidthPc: number,
  isLastInRow: boolean,
  isLastRow: boolean,
}

export default function({
  images,
  containerWidth,
  targetHeight,
  gutter = 2,
}: LayoutParams) {
  // clone the images, and set ratio and initial scaled width / height
  const _images: {
    index: number,
    ratio: number,
    width: number,
    height: number
  }[] = images.map((image, index) => {
    return {
      ...image,
      index,
      ratio: ratio(image.width, image.height)
    };
  });

  const nodeSeekLimit = calcSeekLimit(containerWidth, targetHeight);

  const graph = (start: number) => {
    const results: number[] = [];
    start = +start;
    results[start] = 0;
    for (let i = start + 1; i < _images.length + 1; ++i) {
      if (i - start > nodeSeekLimit) {
        break;
      }
      results[i] = cost( // this was results['' + i] before typescript conversion
        _images,
        start,
        i,
        containerWidth,
        targetHeight,
        gutter
      );
    }
    return results;
  };

  const path = dijkstra.find_path(graph, '0', _images.length);

  const scaledImages: ImageLayout[] = [];
  for (let i = 0; i < path.length; i++) {
    if (path[i + 1]) {
      const row = _images.slice(+path[i], +path[i + 1]);
      const isLastRow = i === path.length - 2;

      // scale row
      const rowHeight = getRowHeight(row, containerWidth, gutter);
      row.forEach((image, index) => {
        const scaledWidth = scaleWidth(rowHeight, image.ratio); //.toFixed(1);
        const scaledHeight = rowHeight;
        const scaledWidthPc = round((scaledWidth / containerWidth) * 100);

        const isLastInRow = index === row.length - 1;

        scaledImages.push({
          scaledWidth: scaledWidth,
          scaledHeight: scaledHeight,
          scaledWidthPc: scaledWidthPc,
          isLastInRow,
          isLastRow,
        });
      });
    }
  }

  return scaledImages;
}
