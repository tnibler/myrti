import type { Dayjs } from 'dayjs';
import type { AssetSeriesRef, TimelineOptions } from './timeline.svelte';
import createJustifiedLayout from 'justified-layout';
import type { TimelineSegment, TimelineBlock, TimelineGridItem } from './timeline-types';
import * as R from 'remeda';
import type { AssetId, AssetSeriesId, AssetWithSpe } from '@api/myrti';
import dayjs from 'dayjs';

type Box = { top: number; left: number; width: number; height: number };

export function layoutSegments(
  segments: TimelineSegment[],
  previousSectionEndDate: Dayjs | null,
  containerWidth: number,
  opts: TimelineOptions,
  getAsset: (id: AssetId) => AssetWithSpe,
  getAssetSeries: (id: AssetSeriesId) => AssetSeriesRef,
): {
  blocks: TimelineBlock[];
} {
  if (segments.length === 0) {
    return { blocks: [] };
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
        const geometry = createJustifiedLayout(assetSizes, {
          targetRowHeight: opts.targetRowHeight,
          containerWidth,
          containerPadding: 0,
          boxSpacing: opts.boxSpacing,
          targetRowHeightTolerance: 0.2,
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

  const blocks: TimelineBlock[] = [];
  let lastMajorTitleDate: Dayjs | null = previousSectionEndDate?.startOf('month') ?? null;

  for (const { segments, height } of mergedSegments) {
    const gridItems: TimelineGridItem[] = R.pipe(
      segments,
      R.flatMap((segment) => R.zip(segment.boxes, segment.segment.items)),
      R.map(([box, item]) => {
        if (item.itemType === 'asset') {
          const gridItem: TimelineGridItem & { type: 'asset' } = {
            type: 'asset',
            top: box.top,
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
            top: box.top,
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

    const locale = 'de';
    const dateFmtYear = new Intl.DateTimeFormat(locale, {
      year: 'numeric',
      month: 'long',
    });
    const dateFmt = new Intl.DateTimeFormat(locale, {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });
    const dateFmtDay = new Intl.DateTimeFormat(locale, {
      month: 'long',
      day: 'numeric',
    });
    const gridHeight = Math.max(...gridItems.map((it) => it.top + it.height));
    if (segments[0].segment.type === 'creatingGroup') {
      console.assert(
        segments.length === 1,
        'creatingGroup segment must not be merged with other segment',
      );
      blocks.push({
        blockType: 'createGroup',
        gridItems,
        gridHeight,
        fullHeight: gridHeight,
        sortDate: dayjs(segments[0].segment.sortDate),
      });
    } else if (segments.length === 1 && segments[0].segment.type === 'group') {
      // segment with only 1 group gets a big title
      const groupSegment = segments[0].segment;
      const lastItem = gridItems[gridItems.length - 1];

      blocks.push({
        blockType: 'default',
        titleMajor: {
          text:
            groupSegment.title +
            (groupSegment.end.startOf('day') == groupSegment.start.startOf('day')
              ? ` (${dateFmtDay.format(groupSegment.start.toDate())})`
              : ` (${dateFmt.formatRange(groupSegment.end.toDate(), groupSegment.start.toDate())})`),
          key: `title-major-group-${groupSegment.groupId}`,
        },
        titlesMinor: [],
        gridItems,
        gridHeight,
        fullHeight: gridHeight,
        sortDate: dayjs(groupSegment.sortDate),
        groupClickAreas: [
          {
            groupId: groupSegment.groupId,
            left: 0,
            top: 0,
            width: lastItem.left + lastItem.width,
            height: gridHeight,
          },
        ],
      });
    } else {
      const firstSegment = segments[0].segment;
      const firstSegmentMonth = firstSegment.end.startOf('month');
      const titleMajor = (() => {
        if (lastMajorTitleDate === null || !lastMajorTitleDate.isSame(firstSegmentMonth)) {
          lastMajorTitleDate = segments[0].segment.start.startOf('month');
          return {
            text: dateFmtYear.format(segments[0].segment.start.toDate()),
            key: segments[0].segment.items[0].sortDate,
          };
        }
        return null;
      })();
      const titlesMinor = segments.map(({ segment, boxes }) => {
        const text = (() => {
          if (segment.type === 'group') {
            return (
              segment.title +
              (segment.end.startOf('day') == segment.start.startOf('day')
                ? ` (${dateFmtDay.format(segment.start.toDate())})`
                : ` (${dateFmt.formatRange(segment.end.toDate(), segment.start.toDate())})`)
            );
          } else {
            return dateFmtDay.format(segment.start.toDate());
          }
        })();
        const lastBox = boxes[boxes.length - 1];
        return {
          text,
          left: boxes[0].left,
          width: lastBox.left + lastBox.width - boxes[0].left,
          key: segments[0].segment.items[0].sortDate,
        };
      });
      blocks.push({
        blockType: 'default',
        titleMajor,
        titlesMinor,
        gridItems,
        gridHeight,
        fullHeight: gridHeight,
        sortDate: dayjs(segments[0].segment.sortDate),
        groupClickAreas: R.pipe(
          segments,
          R.map(({ segment, boxes }) => {
            if (segment.type !== 'group') {
              return null;
            }
            const lastBox = boxes[boxes.length - 1];
            return {
              groupId: segment.groupId,
              left: boxes[0].left,
              top: boxes[0].top,
              width: lastBox.left + lastBox.width - boxes[0].left,
              height: lastBox.top + lastBox.height - boxes[0].top,
            };
          }),
          R.filter(R.isNonNull),
        ),
      });
    }
  }
  const allKeys = R.flatMap(segments, (seg) => R.map(seg.items, (item) => item.key));
  const uniqueKeys = new Set(allKeys);
  console.assert(
    uniqueKeys.size === allKeys.length,
    `Non-unique item keys: ${allKeys.length} but ${uniqueKeys} keys. Duplicates: `,
    R.difference(allKeys, R.unique(allKeys)),
  );
  let blockTop = 0;
  for (const block of blocks) {
    block.top = blockTop;
    blockTop += block.fullHeight;
  }
  return {
    blocks,
  };
}
