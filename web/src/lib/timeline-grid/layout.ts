import type { AssetWithSpe } from '@lib/apitypes';
import type { Dayjs } from 'dayjs';
import type { ItemRange, TimelineGridItem, TimelineOptions } from './timeline.svelte';
import { headerPlugin } from '@zodios/core';
import createJustifiedLayout from 'justified-layout';

type Segment = {
  type: string;
  assets: AssetWithSpe[];
  sortDate: string;
} & (
  | {
      type: 'dateRange';
      start: Dayjs;
      end: Dayjs;
    }
  | { type: 'group'; title: string; groupId: string }
);

type Box = { top: number; left: number; width: number; height: number };

function layoutSegments(
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
    segments: (Segment & { type: 'dateRange' })[];
    width: number;
  } | null = null;
  const interMergedSegmentMargin = 20;
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
      if (segmentIndex === 0 || candidateToMergeWith === null || segment.type !== 'dateRange') {
        return false;
      }
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
      console.assert(segment.type === 'dateRange');
      candidateToMergeWith!.segments.push(segment as Segment & { type: 'daterange' });
      candidateToMergeWith!.width += segmentWidth + interMergedSegmentMargin;
    } else {
      // can not merge with previous segments
      if (candidateToMergeWith !== null) {
        // push candidateToMergeWith items
        console.assert(candidateToMergeWith.width <= containerWidth);
        const mergedRow = [];
        let startLeft = 0;
        for (const segment of candidateToMergeWith.segments) {
          const boxes: Box[] = [];
          for (const asset of segment.assets) {
            const assetSize =
              (asset.rotationCorrection ?? 0) % 180 === 0
                ? { width: asset.width, height: asset.height }
                : { width: asset.height, height: asset.width };
            const boxWidth = assetSize.width * (assetSize.height / opts.targetRowHeight);
            boxes.push({
              top: 0,
              left: startLeft,
              width: boxWidth,
              height: opts.targetRowHeight,
            });
            startLeft += opts.boxSpacing + assetSize.width;
          }
          startLeft -= opts.boxSpacing; // n boxes, n-1 gaps
          mergedRow.push({ segment, boxes });
          startLeft += interMergedSegmentMargin;
        }
        startLeft -= interMergedSegmentMargin; // n boxes, n-1 gaps
        console.assert(startLeft <= containerWidth);
        mergedSegments.push({ segments: mergedRow, height: opts.targetRowHeight });
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
      } else if (segment.type === 'dateRange') {
        // not multiline, might be able to merge with next segment
        candidateToMergeWith = { segments: [segment], width: segmentWidth };
      } else {
        const boxes: Box[] = [];
        let startLeft = 0;
        for (const asset of segment.assets) {
          const assetSize =
            (asset.rotationCorrection ?? 0) % 180 === 0
              ? { width: asset.width, height: asset.height }
              : { width: asset.height, height: asset.width };
          const boxWidth = assetSize.width * (assetSize.height / opts.targetRowHeight);
          boxes.push({
            top: 0,
            left: startLeft,
            width: boxWidth,
            height: opts.targetRowHeight,
          });
          startLeft += opts.boxSpacing + assetSize.width;
        }
        startLeft -= opts.boxSpacing; // n boxes, n-1 gaps
        mergedSegments.push({ segments: [{ segment, boxes }], height: opts.targetRowHeight });
      }
    }
  }
  const items: TimelineGridItem[] = [];
  const segmentItemRanges: ItemRange[] = [];
  console.assert(segmentItemRanges.length === segments.length);
  console.assert(segmentItemRanges.at(-1)!.endIdx === items.length);
  return {
    items: [],
    totalHeight: 0,
    segmentItemRanges: [],
  };
}

/** Creates layout and items representing segments of a section.
 * return value segmentItemRanges contains index ranges into this section's items.
 * arguments are not mutated */
function populateSection(
  segments: TimelineSegment[],
  baseTop: number,
  baseAssetIndex: number,
  containerWidth: number,
): { items: TimelineGridItem[]; sectionHeight: number; segmentItemRanges: ItemRange[] } {
  const targetRowHeight = opts.targetRowHeight;
  const segmentMargin = opts.segmentMargin;
  const items: TimelineGridItem[] = [];
  let nextSegmentTop = baseTop;
  let assetIndex = baseAssetIndex;
  const segmentItemRanges: ItemRange[] = [];
  for (const segment of segments) {
    const itemStartIdx = items.length;
    nextSegmentTop += segmentMargin;
    const headerHeight =
      initialHeightGuess.segmentTitle !== null
        ? initialHeightGuess.segmentTitle
        : opts.headerHeight;
    const title: TimelineGridItem = (() => {
      if (segment.type === 'userGroup') {
        return {
          type: 'segmentTitle',
          titleType: 'major',
          top: nextSegmentTop,
          height: headerHeight,
          key: `group-${segment.data.id}`,
          title: segment.data.name ?? 'Unnamed group',
        };
      }
      if (segment.type === 'dateRange') {
        const start = dayjs(segment.data.start);
        const end = dayjs(segment.data.end);
        let title = '';
        dayjs.extend(localizedFormat);
        if (start.isSame(end, 'day')) {
          title = start.format('MMMM D');
        } else if (start.isSame(end, 'month')) {
          title = start.format('MMMM D') + ' - ' + end.format('D');
        } else {
          title = start.format('MMMM D') + ' - ' + end.format('MMMM D');
        }
        return {
          type: 'segmentTitle',
          titleType: 'major',
          top: nextSegmentTop,
          height: headerHeight,
          key: `${segment.data.start}-${segment.data.end}`,
          title,
        };
      }
      return {
        type: 'createGroupTitleInput',
        top: nextSegmentTop,
        height: headerHeight,
        key: `createGroupTitle${groupNumber}`,
      };
    })();
    items.push(title);
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
    const geometry = createJustifiedLayout(assetSizes, {
      targetRowHeight,
      containerWidth,
      containerPadding: 0,
      boxSpacing: opts.boxSpacing,
    });
    const assetsYMin = nextSegmentTop + title.height + segmentMargin;
    for (let i = 0; i < geometry.boxes.length; i += 1) {
      const box = geometry.boxes[i];
      items.push({
        type: 'asset',
        left: box.left,
        width: box.width,
        top: assetsYMin + box.top,
        height: box.height,
        assetIndex,
        asset: segment.assets[i],
        key: segment.assets[i].id,
      });
      assetIndex += 1;
    }
    nextSegmentTop += geometry.containerHeight + title.height + segmentMargin;
    segmentItemRanges.push({ startIdx: itemStartIdx, endIdx: items.length });
  }

  console.assert(segmentItemRanges.length === segments.length);
  return { items, sectionHeight: nextSegmentTop - baseTop, segmentItemRanges };
}
