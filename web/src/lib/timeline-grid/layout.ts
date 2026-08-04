import type { Dayjs } from 'dayjs';
import type { AssetSeriesRef, TimelineGridItem, TimelineOptions } from './timeline.svelte';
import createJustifiedLayout from 'justified-layout';
import type { TimelineSegment } from './timeline-types';
import * as R from 'remeda';
import type { AssetId, AssetSeriesId, AssetWithSpe } from '@api/myrti';
import { all } from 'axios';

type Box = { top: number; left: number; width: number; height: number };

export function layoutSegments(
  segments: TimelineSegment[],
  previousSectionEndDate: Dayjs | null,
  containerWidth: number,
  opts: TimelineOptions,
  getAsset: (id: AssetId) => AssetWithSpe,
  getAssetSeries: (id: AssetSeriesId) => AssetSeriesRef,
): {
  totalHeight: number;
} {
  if (segments.length === 0) {
    return { totalHeight: 0 };
  }
  for (const segment of segments) {
    segment.gridItems = [];
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

  /** Utility function: next segment was not merged with previously saved merge candidate,
   * so compute layout for saved candidate and add to result array */
  const layoutAndPushMergeCandidate = (candidateToMergeWith: MergeCandidate) => {
    const mergedRow = [];
    let startLeft = 0;
    for (const segment of candidateToMergeWith.segments) {
      const boxes: Box[] = [];
      for (const item of segment.items) {
        const asset =
          item.itemType === 'asset'
            ? getAsset(item.assetId)
            : getAsset(getAssetSeries(item.seriesId).assetIds[item.coverIndex]);
        const file = asset.repFile;
        const assetSize =
          (file.rotationCorrection ?? 0) % 180 === 0
            ? { width: file.width, height: file.height }
            : { width: file.height, height: file.width };
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
      startLeft += opts.segmentMargin;
    }
    startLeft -= opts.segmentMargin; // n boxes, n-1 gaps
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
      const assetId =
        item.itemType === 'asset'
          ? item.assetId
          : getAssetSeries(item.seriesId).assetIds[item.coverIndex];
      const file = getAsset(assetId).repFile;
      return (file.rotationCorrection ?? 0) % 180 === 0
        ? { width: file.width, height: file.height }
        : { width: file.height, height: file.width };
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
    const canMergeWithPrevious = (candidateToMergeWith: MergeCandidate) => {
      if (
        segmentIndex === 0 ||
        candidateToMergeWith === null ||
        // creatingGroup can not merge nor be merged into
        segment.type === 'creatingGroup' ||
        candidateToMergeWith?.segments.at(-1)?.type === 'creatingGroup'
      ) {
        return false;
      }
      console.assert(candidateToMergeWith.segments.length > 0);
      const prevSegment = candidateToMergeWith.segments.at(-1)!;
      const fitsInWidth =
        candidateToMergeWith.width + segmentWidth + opts.segmentMargin <= containerWidth;
      return (
        segment.start.month() === prevSegment.start.month() &&
        segment.start.year() === prevSegment.start.year() &&
        fitsInWidth
      );
    };
    if (candidateToMergeWith && canMergeWithPrevious(candidateToMergeWith)) {
      candidateToMergeWith.segments.push(segment);
      candidateToMergeWith.width += segmentWidth + opts.segmentMargin;
    } else {
      // can not merge with previous segments
      if (candidateToMergeWith !== null) {
        // push candidateToMergeWith items
        console.assert(candidateToMergeWith.width <= containerWidth);
        layoutAndPushMergeCandidate(candidateToMergeWith);
        candidateToMergeWith = null;
      }
      if (segmentWidth > containerWidth) {
        // multiline segment
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
  }
  console.assert(
    segments.length === mergedSegments.reduce((acc: number, s) => acc + s.segments.length, 0),
  );
  let startTop = 0;
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
      segments[0].segment.gridItems.push(titleInput);
      startTop += titleInput.height;
      showMinorTitles = false;
    } else if (segments.length === 1 && segments[0].segment.type === 'group') {
      const group = segments[0].segment;
      const majorTitle: TimelineGridItem = {
        type: 'segmentTitle',
        titleType: 'major',
        top: startTop,
        height: opts.headerHeight,
        title:
          group.title +
          (group.end.startOf('month') == group.start.startOf('month')
            ? ` (${group.end.format('MMMM YYYY')})`
            : ` (${group.end.format('MMMM YYYY')} - ${group.start.format('MMMM YYYY')})`),
        key: 'titleMajorGroup' + group.groupId,
      };
      segments[0].segment.gridItems.push(majorTitle);
      startTop += majorTitle.height;
      showMinorTitles = false;
    } else {
      const firstSegment = segments[0].segment;
      const firstSegmentMonth = firstSegment.end.startOf('month');
      if (lastMajorTitleDate === null || !lastMajorTitleDate.isSame(firstSegmentMonth)) {
        const majorTitle: TimelineGridItem = {
          type: 'segmentTitle',
          titleType: 'major',
          top: startTop,
          height: opts.headerHeight,
          title: segments[0].segment.start.format('MMMM YYYY'),
          key: 'titleMajor' + firstSegmentMonth.format('YYYY-MM') + firstSegment.items[0].sortDate,
        };
        startTop += majorTitle.height;
        lastMajorTitleDate = firstSegmentMonth;
        segments[0].segment.gridItems?.push(majorTitle);
      }
    }

    for (const { segment, boxes } of segments) {
      let offsetByTitleHeight = 0;
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
        offsetByTitleHeight = minorTitle.height;
        segment.gridItems.push(minorTitle);
      }
      segment.gridItems.push(
        ...boxes.map((box, idxInSegment) => {
          const item = segment.items[idxInSegment];
          if (item.itemType === 'asset') {
            const gridItem: TimelineGridItem & { type: 'asset' } = {
              type: 'asset',
              top: box.top + startTop + minorTitleHeight,
              left: box.left,
              width: box.width,
              height: box.height,
              key: 'asset' + item.assetId,
              assetId: item.assetId,
              timelineItem: item,
            };
            return gridItem;
          } else {
            const series = getAssetSeries(item.seriesId);
            const coverAssetId = series.assetIds[item.coverIndex];
            const gridItem: TimelineGridItem & { type: 'photoStack' } = {
              type: 'photoStack',
              top: box.top + startTop + offsetByTitleHeight,
              left: box.left,
              width: box.width,
              height: box.height,
              key: 'asset' + coverAssetId, // no thought behind this
              seriesId: item.seriesId,
              coverIndex: item.coverIndex,
              numAssets: series.assetIds.length,
              timelineItem: item,
            };
            return gridItem;
          }
        }),
      );
    }
    minorTitleRowIdx += 1;
    startTop += minorTitleHeight + height;
  }
  const allKeys = R.flatMap(segments, (seg) => R.map(seg.items, (item) => item.key));
  const uniqueKeys = new Set(allKeys);
  console.assert(
    uniqueKeys.size === allKeys.length,
    `Non-unique item keys: ${allKeys.length} but ${uniqueKeys} keys. Duplicates: `,
    R.difference(allKeys, R.unique(allKeys)),
  );
  return {
    totalHeight: startTop,
  };
}
