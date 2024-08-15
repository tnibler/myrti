import type { Dayjs } from 'dayjs';
import type { TimelineGridItem, TimelineOptions } from './timeline.svelte';
import createJustifiedLayout from 'justified-layout';
import type { ItemRange, TimelineSegment } from './timeline-types';

type Box = { top: number; left: number; width: number; height: number };

export function layoutSegments(
  segments: TimelineSegment[],
  previousSectionEndDate: Dayjs | null,
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
  // First, merge any segments (too short to fill a line) and compute their layouts
  const mergedSegments: {
    segments: {
      segment: TimelineSegment;
      /** Layout boxes for this segment's items starting from top=0, but including inter-segment margins */
      boxes: Box[];
    }[];
    /** Total height of this row (which only has asset boxes, no titles) */
    height: number;
  }[] = [];
  type MergeCandidate = {
    segments: TimelineSegment[];
    width: number;
  };
  /** save last segment if it might be merged with the next one */
  let candidateToMergeWith: MergeCandidate | null = null;
  const interMergedSegmentMargin = 20;

  /** Utility function: next segment was not merged with previously saved merge candidate,
   * so compute layout for saved candidate and add to result array */
  const layoutAndPushMergeCandidate = (candidateToMergeWith: MergeCandidate) => {
    const mergedRow = [];
    let startLeft = 0;
    for (const segment of candidateToMergeWith.segments) {
      const boxes: Box[] = [];
      for (const item of segment.items) {
        const asset = item.itemType === 'asset' ? item : item.series.assets[item.coverIndex];
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
    // swap width/heigth if rotation correction applies
    const assetSizes = segment.items.map((item) => {
      const asset = item.itemType === 'asset' ? item : item.series.assets[item.coverIndex];
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
        // scale down to fit targetRowHeight
        .map((sz) => sz.width * (opts.targetRowHeight / sz.height))
        // sum up to total width
        .reduce((acc, n) => acc + n, 0) +
      // with spacing between boxes
      (assetSizes.length - 1) * opts.boxSpacing;
    const canMergeWithPrevious: boolean = (() => {
      if (segmentIndex === 0 || candidateToMergeWith === null) {
        return false;
      }
      if (
        segment.type === 'creatingGroup' ||
        candidateToMergeWith?.segments.at(-1)?.type === 'creatingGroup'
      ) {
        // creatingGroup can not merge nor be merged into
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
      candidateToMergeWith!.segments.push(segment);
      candidateToMergeWith!.width += segmentWidth + interMergedSegmentMargin;
    } else {
      // can not merge with previous segments
      if (candidateToMergeWith !== null) {
        // push candidateToMergeWith items
        console.assert(candidateToMergeWith.width <= containerWidth);
        layoutAndPushMergeCandidate(candidateToMergeWith);
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
    layoutAndPushMergeCandidate(candidateToMergeWith);
    candidateToMergeWith = null;
  }
  console.assert(
    segments.length === mergedSegments.reduce((acc, s) => (acc += s.segments.length), 0),
  );
  const items: TimelineGridItem[] = [];
  const segmentItemRanges: ItemRange[] = [];
  let startTop = baseTop;
  const minorTitleHeight = 10;
  let lastMajorTitleDate: Dayjs | null = previousSectionEndDate?.startOf('month') ?? null;
  let minorTitleRowIdx = 0;
  for (const { segments, height } of mergedSegments) {
    let showMinorTitles = true;
    if (segments[0].segment.type === 'creatingGroup') {
      console.assert(
        segments.length === 1,
        'creatingGroup segment must not be merged with other segment',
      );
      const titleInput: TimelineGridItem = {
        type: 'createGroupTitleInput',
        top: startTop,
        height: opts.headerHeight,
        key: 'createGroupTitleInput',
      };
      items.push(titleInput);
      startTop += titleInput.height;
      showMinorTitles = false;
    } else {
      const firstSegment = segments[0].segment;
      const firstSegmentMonth = firstSegment.start.startOf('month');
      if (lastMajorTitleDate === null || !lastMajorTitleDate.isSame(firstSegmentMonth)) {
        const majorTitle: TimelineGridItem = {
          type: 'segmentTitle',
          titleType: 'major',
          top: startTop,
          height: opts.headerHeight,
          title: segments[0].segment.start.format('MMMM YYYY'),
          key: 'titleMajor' + firstSegmentMonth.format('YYYY-MM'), // broken because of duplicate months
        };
        items.push(majorTitle);
        startTop += majorTitle.height;
        lastMajorTitleDate = firstSegmentMonth;
      }
    }
    for (const { segment, boxes } of segments) {
      const startItemIndex = items.length;
      let minorTitleHeight = 0;
      if (showMinorTitles) {
        const minorTitle: TimelineGridItem = {
          type: 'segmentTitle',
          titleType: 'day',
          title: segment.type === 'group' ? segment.title : segment.start.format('MMMM Do'),
          top: startTop,
          height: minorTitleHeight,
          left: boxes[0].left,
          width: boxes.at(-1)!.left + boxes.at(-1)!.width - boxes[0].left,
          titleRowIndex: minorTitleRowIdx,
          key:
            'titleMinor' +
            (segment.type === 'group' ? 'group' + segment.groupId : segment.start.format()),
        };
        minorTitleHeight = minorTitle.height;
        items.push(minorTitle);
      }
      items.push(
        ...boxes.map((box, idxInSegment) => {
          const item = segment.items[idxInSegment];
          if (item.itemType === 'asset') {
            const gridItem: TimelineGridItem & { type: 'asset' } = {
              type: 'asset',
              top: box.top + startTop + minorTitleHeight,
              left: box.left,
              width: box.width,
              height: box.height,
              key: 'asset' + item.id,
              asset: item,
              assetIndex: item.assetIndex,
            };
            return gridItem;
          } else {
            const coverAsset = item.series.assets[item.coverIndex];
            const gridItem: TimelineGridItem & { type: 'photoStack' } = {
              type: 'photoStack',
              top: box.top + startTop + minorTitleHeight,
              left: box.left,
              width: box.width,
              height: box.height,
              key: 'asset' + coverAsset.id, // no thought behind this
              series: item.series,
              coverIndex: item.coverIndex,
              numAssets: item.series.assets.length,
              firstAssetIndex: item.firstAssetIndex,
            };
            return gridItem;
          }
        }),
      );
      const endItemIndex = items.length;
      segmentItemRanges.push({ startIdx: startItemIndex, endIdx: endItemIndex });
    }
    minorTitleRowIdx += 1;
    startTop += minorTitleHeight + height;
  }
  const uniqueKeys = new Set(items.map((i) => i.key)).size;
  console.assert(
    uniqueKeys === items.length,
    `Non-unique item keys: ${items.length} but ${uniqueKeys} keys`,
  );
  console.assert(segmentItemRanges.length === segments.length);
  console.assert(segmentItemRanges.at(-1)!.endIdx === items.length);
  return {
    items,
    segmentItemRanges,
    totalHeight: startTop - baseTop,
  };
}
