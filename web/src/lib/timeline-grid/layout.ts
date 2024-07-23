import type { AssetWithSpe } from '@lib/apitypes';
import type { Dayjs } from 'dayjs';
import type { ItemRange, TimelineGridItem, TimelineOptions } from './timeline.svelte';
import createJustifiedLayout from 'justified-layout';

export type Segment = {
  type: string;
  assets: AssetWithSpe[];
  sortDate: string;
  start: Dayjs;
  end: Dayjs;
} & (
  | {
      type: 'dateRange';
    }
  | { type: 'group'; title: string; groupId: string }
);

type Box = { top: number; left: number; width: number; height: number };

export function layoutSegments(
  segments: Segment[],
  baseTop: number,
  baseAssetIndex: number,
  containerWidth: number,
  opts: TimelineOptions,
): {
  items: TimelineGridItem[];
  totalHeight: number;
  segmentItemRanges: ItemRange[];
} {
  if (segments.length === 0) {
    return {
      items: [],
      totalHeight: 0,
      segmentItemRanges: [],
    };
  }
  const mergedSegments: {
    segments: {
      segment: Segment;
      /** Layout boxes for this segment's items starting from top=0, but including inter-segment margins */
      boxes: Box[];
    }[];
    /** Total height of this row (which only has asset boxes, no titles) */
    height: number;
  }[] = [];
  let candidateToMergeWith: {
    segments: Segment[];
    width: number;
  } | null = null;
  const interMergedSegmentMargin = 20;

  const layoutAndPushMergeCandidate = () => {
    const mergedRow = [];
    let startLeft = 0;
    for (const segment of candidateToMergeWith.segments) {
      const boxes: Box[] = [];
      for (const asset of segment.assets) {
        const assetSize =
          (asset.rotationCorrection ?? 0) % 180 === 0
            ? { width: asset.width, height: asset.height }
            : { width: asset.height, height: asset.width };
        const boxWidth = assetSize.width * (opts.targetRowHeight / assetSize.height);
        boxes.push({
          top: 0,
          left: startLeft,
          width: boxWidth,
          height: opts.targetRowHeight,
        });
        startLeft += opts.boxSpacing + boxWidth;
      }
      startLeft -= opts.boxSpacing; // n boxes, n-1 gaps
      mergedRow.push({ segment, boxes });
      startLeft += interMergedSegmentMargin;
    }
    startLeft -= interMergedSegmentMargin; // n boxes, n-1 gaps
    console.assert(
      startLeft <= containerWidth,
      `after laying out row, startLeft should be <= ${containerWidth} but is ${startLeft}`,
    );
    mergedSegments.push({ segments: mergedRow, height: opts.targetRowHeight });
  };

  for (let segmentIndex = 0; segmentIndex < segments.length; segmentIndex += 1) {
    const segment = segments[segmentIndex];
    const assetSizes = segment.assets.map((asset) => {
      if (asset.rotationCorrection && asset.rotationCorrection % 180 != 0) {
        return {
          width: asset.height,
          height: asset.width,
        };
      } else {
        return {
          width: asset.width,
          height: asset.height,
        };
      }
    });
    // mergeable if all of:
    //  - previous segment does not fill at least one line
    //  - current and previous fit on one line
    //  - current and previous are of same month and year
    const segmentWidth =
      assetSizes
        .map((sz) => sz.width * (opts.targetRowHeight / sz.height))
        .reduce((acc, n) => acc + n, 0) +
      (assetSizes.length - 1) * opts.boxSpacing;
    const canMergeWithPrevious: boolean = (() => {
      if (segmentIndex === 0 || candidateToMergeWith === null) {
        return false;
      }
      // if (
      //   segment.type === 'group' &&
      //   segment.start.startOf('month') !== segment.end.startOf('month')
      // ) {
      //   return false;
      // }
      console.assert(candidateToMergeWith.segments.length > 0);
      const prevSegment = candidateToMergeWith.segments.at(-1)!;
      const sameMonthAndYear =
        segment.start.month() === prevSegment.start.month() &&
        segment.start.year() === prevSegment.start.year();
      const fitsInWidth =
        candidateToMergeWith.width + segmentWidth + interMergedSegmentMargin <= containerWidth;
      return sameMonthAndYear && fitsInWidth;
    })();
    if (canMergeWithPrevious) {
      console.assert(candidateToMergeWith !== null && candidateToMergeWith.segments.length > 0);
      candidateToMergeWith!.segments.push(segment);
      candidateToMergeWith!.width += segmentWidth + interMergedSegmentMargin;
    } else {
      // can not merge with previous segments
      if (candidateToMergeWith !== null) {
        // push candidateToMergeWith items
        console.assert(candidateToMergeWith.width <= containerWidth);
        layoutAndPushMergeCandidate();
        candidateToMergeWith = null;
      }
      const isMultiline = segmentWidth > containerWidth;
      if (isMultiline) {
        // justified layout
        const geometry = createJustifiedLayout(assetSizes, {
          targetRowHeight: opts.targetRowHeight,
          containerWidth,
          containerPadding: 0,
          boxSpacing: opts.boxSpacing,
        });
        mergedSegments.push({
          segments: [{ segment, boxes: geometry.boxes }],
          height: geometry.containerHeight,
        });
      } else {
        // not multiline, might be able to merge with next segment
        candidateToMergeWith = { segments: [segment], width: segmentWidth };
      }
    }
  }
  if (candidateToMergeWith !== null) {
    layoutAndPushMergeCandidate();
    candidateToMergeWith = null;
  }
  console.assert(
    segments.length === mergedSegments.reduce((acc, s) => (acc += s.segments.length), 0),
  );
  const items: TimelineGridItem[] = [];
  const segmentItemRanges: ItemRange[] = [];
  let startTop = baseTop;
  const minorTitleHeight = 50;
  let lastMajorTitleDate: Dayjs | null = null;
  for (const { segments, height } of mergedSegments) {
    const firstSegment = segments[0].segment;
    const firstSegmentMonth = firstSegment.start.startOf('month');
    if (lastMajorTitleDate === null || !lastMajorTitleDate.isSame(firstSegmentMonth)) {
      lastMajorTitleDate = firstSegmentMonth;
      const majorTitle: TimelineGridItem = {
        type: 'segmentTitle',
        titleType: 'major',
        top: startTop,
        height: minorTitleHeight,
        title: segments[0].segment.start.format('MMMM YYYY'),
        key: 'titleMajor' + firstSegmentMonth.format('YYYY-MM'),
      };
      items.push(majorTitle);
      startTop += majorTitle.height;
    }
    let startAssetIndex = baseAssetIndex;
    for (const { segment, boxes } of segments) {
      const startItemIndex = items.length;
      const minorTitle: TimelineGridItem = {
        type: 'segmentTitle',
        titleType: 'day',
        title: segment.type === 'group' ? segment.title : segment.start.format('MMMM Do'),
        top: startTop,
        height: minorTitleHeight,
        left: boxes[0].left,
        width: boxes.at(-1)!.left + boxes.at(-1)!.width - boxes[0].left,
        key:
          'titleMinor' +
          (segment.type === 'group' ? 'group' + segment.groupId : segment.start.format()),
      };
      items.push(minorTitle);
      items.push(
        ...boxes.map((box, idxInSegment) => {
          const asset = segment.assets[idxInSegment];
          const item: TimelineGridItem & { type: 'asset' } = {
            type: 'asset',
            top: box.top + startTop + minorTitleHeight,
            left: box.left,
            width: box.width,
            height: box.height,
            key: 'asset' + asset.id,
            asset,
            assetIndex: startAssetIndex + idxInSegment,
          };
          return item;
        }),
      );
      startAssetIndex += segment.assets.length;
      const endItemIndex = items.length;
      segmentItemRanges.push({ startIdx: startItemIndex, endIdx: endItemIndex });
    }
    startTop += minorTitleHeight + height;
  }
  console.assert(segmentItemRanges.length === segments.length);
  console.assert(segmentItemRanges.at(-1)!.endIdx === items.length);
  return {
    items,
    segmentItemRanges,
    totalHeight: startTop,
  };
}
