import type {
  AssetId,
  AssetWithSpe,
  TimelineSection as ApiTimelineSection,
  TimelineSegment as ApiTimelineSegment,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  TimelineItem as ApiTimelineItem,
} from '@api/myrti';
import { dayjs } from '@lib/dayjs';
import { klona } from 'klona/json';
import { SvelteMap } from 'svelte/reactivity';
import { layoutSegments } from './layout';
import * as R from 'remeda';
import {
  addToTimelineGroup,
  createTimelineGroup,
  getTimelineSections,
  getTimelineSegments,
  setAssetsHidden,
} from '../../api/myrti';
import {
  createTimelineGroupResponse,
  getTimelineSectionsResponse,
  getTimelineSegmentsResponse,
} from '../../api/myrti.zod';
import type {
  AddToGroupClickArea,
  ItemRange,
  AssetSeries,
  TimelineItem,
  TimelineSection,
  TimelineSegment,
} from './timeline-types';

/** A component displayed in the timeline */
export type TimelineGridItem = { key: string; top: number; height: number } & (
  | {
      type: 'asset';
      left: number;
      width: number;
      asset: AssetWithSpe;
      timelineItem: TimelineItem;
    }
  | {
      type: 'photoStack';
      left: number;
      width: number;
      series: AssetSeries;
      coverIndex: number;
      numAssets: number;
      timelineItem: TimelineItem;
    }
  | {
      type: 'segmentTitle';
      titleType: 'major';
      title: string;
    }
  | {
      type: 'segmentTitle';
      titleType: 'day';
      title: string;
      left: number;
      width: number;
      titleRowIndex: number;
    }
  | {
      type: 'createGroupTitleInput';
    }
);

/** Backlink from TimelineItem to its segment/section */
export type PositionInTimeline = {
  sectionIndex: number;
  segmentIndex: number;
  itemIndex: number;
};

export type Viewport = { width: number; height: number };

export interface ITimelineGrid {
  readonly state: 'justLooking' | 'creatingTimelineGroup';
  readonly totalNumAssets: number;
  /** All items, in same order as sections with non-decreasing `top` */
  readonly items: TimelineGridItem[];
  readonly sections: TimelineSection[];
  readonly timelineHeight: number;
  /** Range of indices into items corresponding to currently visible section*/
  readonly visibleItems: ItemRange;
  readonly options: TimelineOptions;
  readonly addToGroupClickAreas: AddToGroupClickArea[];
  // /** Assets are highlighted when something is selected and shift is pressed to preview
  //  * possible range selection. */
  // readonly selectionPreviewIds: Map<AssetId, boolean>;

  initialize: (viewport: Viewport) => Promise<void>;
  resize: (viewport: Viewport, scrollTop: number) => void;
  set setAnimationsEnabled(v: ((enabled: boolean) => Promise<void>) | null);
  onScrollChange: (top: number) => void;
  getGridItemAtPosition: (pos: PositionInTimeline) => Promise<TimelineGridItem | null>;
  setActualItemHeight: (itemIndex: number, newHeight: number) => void;
  getNextItemPosition: (
    pos: PositionInTimeline,
    dir: 'left' | 'right',
  ) => PositionInTimeline | null;
  getItem: (pos: PositionInTimeline) => Promise<TimelineItem>;
  /** previous/newer item */
  clearSelection: () => void;
  setItemSelected: (item: TimelineItem, selected: boolean) => void;
  isItemSelected: (item: TimelineItem) => boolean;
  readonly numAssetsSelected: number;
  // /** @param clickedAssetIndex asset clicked to perform range selection */
  // setRangeSelected: (clickedAssetIndex: number, selected: boolean) => void;
  // /** Asset is hoevered while shift is pressed, selection range should be highlighted */
  // rangeSelectHover: (hoveredAssetIndex: number) => void;
  hideSelectedAssets: () => Promise<void>;

  createGroupClicked: () => Promise<void>;
  cancelCreateGroup: () => Promise<void>;
  confirmCreateGroup: (title: string) => Promise<void>;
  addSelectedToExistingGroup: (groupId: string) => Promise<void>;
}

export type TimelineOptions = {
  targetRowHeight: number;
  headerHeight: number;
  segmentMargin: number;
  boxSpacing: number;
  loadWithinMargin: number;
};

type TimelineState =
  | { state: 'justLooking' }
  | {
      state: 'creatingTimelineGroup';
      itemsInGroup: TimelineItem[];
      groupSortDate: string;
      previousItems: TimelineGridItem[];
      previousSections: TimelineSection[];
    };

export function createTimeline(
  opts: TimelineOptions,
  adjustScrollTop: (params: {
    what: 'scrollBy' | 'scrollTo';
    scroll: number;
    ifScrollTopGt: number;
    behavior: 'smooth' | 'instant';
  }) => void,
): ITimelineGrid {
  let isInitialized = false;
  let viewport: Viewport = { width: 0, height: 0 };
  let state: TimelineState = $state({ state: 'justLooking' });
  let items: TimelineGridItem[] = $state([]);
  let sections: TimelineSection[] = $state([]);
  const timelineHeight: number = $derived(
    sections.map((s) => s.height).reduce((acc, n) => acc + n, 0),
  );
  const addToGroupClickAreas: AddToGroupClickArea[] = $derived(
    state.state === 'creatingTimelineGroup'
      ? sections
          .filter((s) => s.segments !== null && s.items != null)
          .map((s) =>
            s
              .segments!.filter((seg) => seg.type === 'group')
              .map((seg) => seg.clickArea)
              .filter((area) => area !== null),
          )
          .flat()
      : [],
  );
  let visibleItems: ItemRange = $state({ startIdx: 0, endIdx: 0 });
  let setAnimationsEnabled: ((enabled: boolean) => Promise<void>) | null = null;
  /** maps item key string to index in selection */
  const selectedItems: Map<string, { item: TimelineItem; idx: number }> = $state(new SvelteMap());
  const totalNumAssets: number = $derived(
    sections.reduce((acc: number, section) => {
      return acc + section.data.numAssets;
    }, 0),
  );
  /** Initially, values from TimelineOptions (e.g, headerHeight) are used to set the height of items like segment titles of which we don't know the real size of rendered text.
   * When setRealItemHeight is called, we correct that guess and use it for future items of the same type so that setRealItemHeight needs to be called and relayout everything less often. */
  const initialHeightGuess: Record<string, number | null> = {
    segmentTitle: null,
  };

  const inflightSegmentRequests: Map<string, Promise<ApiTimelineSegment[]>> = new Map();
  function requestSegments(sectionId: string): Promise<ApiTimelineSegment[]> {
    const inflight = inflightSegmentRequests.get(sectionId);
    if (inflight) {
      return inflight;
    } else {
      const insertPromise = (async () => {
        const r = getTimelineSegmentsResponse.parse((await getTimelineSegments(sectionId)).data);
        return r.segments;
      })();
      inflightSegmentRequests.set(sectionId, insertPromise);
      insertPromise.then(() => {
        inflightSegmentRequests.delete(sectionId);
      });
      return insertPromise;
    }
  }

  async function initialize(vp: Viewport) {
    if (isInitialized) {
      return;
    }
    isInitialized = true;
    viewport = { ...vp };
    await loadSectionPlaceholders();
  }

  async function loadSectionPlaceholders() {
    const sectionsResponse = getTimelineSectionsResponse.parse((await getTimelineSections()).data);
    const sectionData: ApiTimelineSection[] = sectionsResponse.sections;

    const _sections: TimelineSection[] = [];
    let nextSectionTop = 0;
    for (const section of sectionData) {
      const height = estimateHeight(section, viewport.width, opts.targetRowHeight);
      _sections.push({
        data: section,
        height,
        top: nextSectionTop,
        segments: null,
        items: null,
        startDate: dayjs.utc(section.startDate),
        endDate: dayjs.utc(section.endDate),
      });
      nextSectionTop += height;
    }
    sections = _sections;
  }

  function resize(newViewport: Viewport, scrollTop: number) {
    if (viewport === newViewport) {
      return;
    }
    viewport = { ...newViewport };
    onScrollChange(scrollTop, true);
  }

  let lastScrollTime: number | null = null;
  async function onScrollChange(top: number, forceRelayout: boolean = false) {
    const loadWithinMargin = opts.loadWithinMargin;
    let firstVisibleSection = null;
    let lastVisibleSection = null;
    for (let i = 0; i < sections.length; i += 1) {
      const s = sections[i];
      const isVisible =
        s.top <= top + viewport.height + loadWithinMargin &&
        top - loadWithinMargin <= s.top + s.height;
      if (firstVisibleSection == null && isVisible) {
        firstVisibleSection = i;
      } else if (i == sections.length - 1 && isVisible) {
        // last section is visible
        lastVisibleSection = i;
      } else if (firstVisibleSection != null && !isVisible) {
        // this section is not visible anymore, previous is last visible one
        // i is at least 1 here
        lastVisibleSection = i - 1;
        break;
      }
    }
    if (lastVisibleSection == null) {
      lastVisibleSection = firstVisibleSection;
    }
    if (firstVisibleSection == null || lastVisibleSection == null) {
      console.error('first and lastVisibleSection are null');
      return;
    }
    const sectionLoads = [];
    for (let i = firstVisibleSection; i <= lastVisibleSection; i += 1) {
      sectionLoads.push(loadSection(i));
    }
    const now = Date.now();
    lastScrollTime = now;
    await Promise.all(sectionLoads);
    if (lastScrollTime != now) {
      return;
    }
    // set visibleItems indices to range from first to last visible section,
    // after waiting for all sections to load. We could do this progressively after any one section loads, but that makes things more complicated for little reason
    // TODO: make the above irrelevant by adding an API call to load multiple sections at once, for the rare event that the user jumps exactly inbetween two sections.
    const fvs = sections[firstVisibleSection];
    const lvs = sections[lastVisibleSection];
    for (let i = firstVisibleSection; i <= lastVisibleSection; i += 1) {
      if (sections[i].items === null || forceRelayout) {
        layoutSection(i, 'adjustScroll');
      }
    }
    visibleItems = {
      // layoutSection populates items, so the field is not null here
      startIdx: fvs.items!.startIdx,
      endIdx: lvs.items!.endIdx,
    };
  }

  function layoutSection(sectionIndex: number, adjustScroll: 'adjustScroll' | 'noAdjustScroll') {
    const section = sections[sectionIndex];
    const segments = section.segments;
    if (segments === null) {
      console.error('sections[sectionIndex].segments must not be null in layoutSection()');
      return;
    }
    if (section.items != null) {
      // section is already laid out
      const numItems = section.items.endIdx - section.items.startIdx;
      // remove existing items
      items.splice(section.items.startIdx, numItems);
      // shift ItemRanges of subsequent sections
      for (let i = sectionIndex + 1; i < sections.length; i += 1) {
        const ir = sections[i].items;
        if (ir === null) {
          continue;
        }
        ir.startIdx -= numItems;
        ir.endIdx -= numItems;
      }
      section.items = null;
      if (section.segments != null) {
        for (const segment of section.segments) {
          segment.itemRange = null;
        }
      }
    }
    let baseAssetIndex = 0;
    for (let i = 0; i < sectionIndex; i += 1) {
      baseAssetIndex += sections[i].data.numAssets;
    }
    const lastSectionEndDate = sectionIndex === 0 ? null : sections[sectionIndex - 1].endDate;
    const {
      items: sectionItems,
      totalHeight: sectionHeight,
      segmentItemRanges,
    } = layoutSegments(
      segments,
      lastSectionEndDate,
      section.top,
      baseAssetIndex,
      viewport.width,
      opts,
    );
    const oldSectionHeight = sections[sectionIndex].height;
    section.height = sectionHeight;
    for (let i = 0; i < segments.length; i += 1) {
      const segment = segments[i];
      // item indices relative to this section's startIdx
      segment.itemRange = segmentItemRanges[i];
      // set group's click area
      if (segment.type === 'group') {
        let currentTop = Infinity;
        let currentBottom = -Infinity;
        let currentLeft = 0;
        let currentRight = viewport.width;
        for (let i = segment.itemRange.startIdx; i < segment.itemRange.endIdx; i += 1) {
          const item = sectionItems[i];
          currentTop = Math.min(item.top, currentTop);
          currentBottom = Math.max(item.top + item.height, currentBottom);
          if (
            item.type === 'asset' ||
            item.type === 'photoStack' ||
            (item.type === 'segmentTitle' && item.titleType === 'day')
          ) {
            currentLeft = Math.min(item.left, currentLeft);
            currentRight = Math.max(item.left + item.width, currentRight);
          }
        }
        segment.clickArea = {
          gridItems: sectionItems.slice(segment.itemRange.startIdx, segment.itemRange.endIdx),
          groupId: segment.groupId,
        };
      }
    }

    // last loaded section before sectionIndex, to insert new items after its ItemRange
    const sectionBefore = sections.findLast((s, i) => i < sectionIndex && s.items != null);
    // insert items inbetween previous loaded and next section
    const insertAtIndex = sectionBefore?.items?.endIdx ?? 0;
    items.splice(insertAtIndex, 0, ...sectionItems);
    sections[sectionIndex].items = {
      startIdx: insertAtIndex,
      endIdx: insertAtIndex + sectionItems.length,
    };
    // Correct sections after newly inserted one: shift top and ItemRange indices
    const heightDelta = sectionHeight - oldSectionHeight;
    for (let i = sectionIndex + 1; i < sections.length; i += 1) {
      const s = sections[i];
      s.top += heightDelta;
      if (s.items) {
        s.items.startIdx += sectionItems.length;
        s.items.endIdx += sectionItems.length;
        for (let i = s.items.startIdx; i < s.items.endIdx; i += 1) {
          items[i].top += heightDelta;
        }
      }
    }
    if (adjustScroll === 'adjustScroll') {
      adjustScrollTop({
        what: 'scrollBy',
        scroll: heightDelta,
        ifScrollTopGt: sections[sectionIndex].top,
        behavior: 'instant',
      });
    }
  }

  async function loadSection(sectionIndex: number) {
    const section = sections[sectionIndex];
    if (section.segments != null) {
      return;
    }
    const sectionId = section.data.id;
    const segments = await requestSegments(sectionId);

    sections[sectionIndex].segments = R.pipe(
      segments,
      R.map((segment, segmentIndex) => {
        // split up stacks with multiple selection images. stacks with multiple selections are shown
        // as multiple items, one for each selection image
        const itemWithStacksSplitUp: TimelineItem[] = [];
        let itemIndex = 0;
        for (const item of segment.items) {
          if (item.itemType === 'asset') {
            itemWithStacksSplitUp.push({
              ...item,
              key: `a-${item.id}`,
              sortDate: item.takenDate,
              pos: { sectionIndex, segmentIndex, itemIndex },
            });
            itemIndex += 1;
          } else {
            // item.itemType === 'photoSeries'
            const series: AssetSeries = {
              assets: item.assets,
              seriesId: item.seriesId,
              selectionIndices: item.selectionIndices,
            };
            // Say we have a series of assets - with selection o
            // --o-o--o-
            // it will get split as --o-    o--   o-
            // So the first (from right to left) splits at each selectionIndex have to increment currentAssetIndex by 1,
            // but the last one also includes the tail (off to the left)
            for (const [idxOfIdx, selectionIdx] of item.selectionIndices.entries()) {
              const isLast = idxOfIdx === item.selectionIndices.length - 1;
              const isFirst = idxOfIdx === 0;
              const splitStart = isFirst ? 0 : selectionIdx;
              const splitEnd = isLast ? item.assets.length : item.selectionIndices[idxOfIdx + 1];
              // sortDate is the date of newest asset in this split of the stack
              const sortDate = item.assets[splitStart].takenDate;
              itemWithStacksSplitUp.push({
                key: `s-${series.seriesId}-${splitStart}-${splitEnd}`,
                itemType: 'photoStack',
                coverIndex: selectionIdx,
                series,
                pos: { sectionIndex, segmentIndex, itemIndex },
                sortDate,
                splitStart,
                splitEnd,
              });
              itemIndex += 1;
            }
          }
        }
        if (segment.type === 'dateRange') {
          return {
            type: 'dateRange' as const,
            items: itemWithStacksSplitUp,
            sortDate: segment.sortDate,
            itemRange: null,
            start: dayjs.utc(segment.start),
            end: dayjs.utc(segment.end),
          };
        } else if (segment.type === 'userGroup') {
          console.assert(segment.items.length > 0);
          if (segment.items.length === 0) {
            return null;
          }
          // get start and end dates from either assets or first/last assets in stack
          const startDate = (() => {
            const item = segment.items[0]; // checked to have at least 1 el above
            const asset = item.itemType === 'asset' ? item : item.assets.at(0);
            return asset?.takenDate;
          })();
          const endDate = (() => {
            const item = segment.items.at(-1); // checked to have at least 1 el above
            const asset = item?.itemType === 'asset' ? item : item?.assets.at(-1);
            return asset?.takenDate;
          })();
          if (startDate === undefined || endDate === undefined) {
            return null;
          }
          return {
            type: 'group' as const,
            title: segment.name ?? 'Unnamed group',
            groupId: segment.id,
            items: itemWithStacksSplitUp,
            sortDate: segment.sortDate,
            itemRange: null,
            clickArea: null,
            start: dayjs.utc(startDate),
            end: dayjs.utc(endDate),
          };
        }
        return null;
      }),
      R.filter(R.isNonNull),
    );
  }

  function isItemSelected(item: TimelineItem): boolean {
    return selectedItems.has(item.key);
  }

  /** Increasing number to track order in which assets are selected. Used for values of selectedAssets */
  let nextSelectionIndex = 0;
  function setItemSelected(item: TimelineItem, selected: boolean) {
    if (item.itemType === 'photoStack') {
      // stack may be split into multiple grid items, and selecting one should select all of them
      const section = sections[item.pos.sectionIndex];
      if (section.segments === null) {
        console.error('timeline: (de)selected items in section that is not loaded');
        return;
      }
      const segment = section.segments[item.pos.segmentIndex];
      const itemsOfSameSeries: (TimelineItem & { itemType: 'photoStack' })[] = [];
      for (let i = item.pos.itemIndex; i < segment.items.length; i += 1) {
        const it = segment.items[i];
        if (it.itemType === 'photoStack' && it.series.seriesId === item.series.seriesId) {
          itemsOfSameSeries.push(it);
        } else {
          break;
        }
      }
      for (let i = item.pos.itemIndex - 1; 0 <= i; i -= 1) {
        const it = segment.items[i];
        if (it.itemType === 'photoStack' && it.series.seriesId === item.series.seriesId) {
          itemsOfSameSeries.push(it);
        } else {
          break;
        }
      }

      if (selected) {
        for (const item of itemsOfSameSeries) {
          selectedItems.set(item.key, { item, idx: nextSelectionIndex });
          nextSelectionIndex += 1;
        }
      } else {
        for (const item of itemsOfSameSeries) {
          selectedItems.delete(item.key);
        }
      }
    } else {
      if (selected) {
        selectedItems.set(item.key, { item, idx: nextSelectionIndex });
        nextSelectionIndex += 1;
      } else {
        selectedItems.delete(item.key);
      }
    }
  }

  function clearSelection() {
    nextSelectionIndex = 0;
    selectedItems.clear();
  }

  function getNextItemPosition(
    pos: PositionInTimeline,
    dir: 'left' | 'right',
  ): PositionInTimeline | null {
    const section = sections[pos.sectionIndex];
    if (section.segments === null) {
      console.error('timeline getNextItemPosition: section is not loaded');
      return null;
    }
    const segment = section.segments[pos.segmentIndex];
    if (dir === 'right') {
      if (pos.itemIndex < segment.items.length - 1) {
        return { ...pos, itemIndex: pos.itemIndex + 1 };
      } else if (pos.segmentIndex < section.segments.length - 1) {
        return { sectionIndex: pos.sectionIndex, segmentIndex: pos.segmentIndex + 1, itemIndex: 0 };
      } else if (pos.sectionIndex < sections.length - 1) {
        return { sectionIndex: pos.sectionIndex + 1, segmentIndex: 0, itemIndex: 0 };
      } else {
        return null;
      }
    } else {
      if (0 < pos.itemIndex) {
        return { ...pos, itemIndex: pos.itemIndex - 1 };
      } else if (0 < pos.segmentIndex) {
        const segs = section.segments;
        const si = pos.segmentIndex - 1;
        return {
          sectionIndex: pos.sectionIndex,
          segmentIndex: si,
          itemIndex: segs[si].items.length - 1,
        };
      } else if (0 < pos.sectionIndex) {
        const segs = section.segments;
        return {
          sectionIndex: pos.sectionIndex - 1,
          segmentIndex: segs.length - 1,
          itemIndex: segs[segs.length - 1].items.length - 1,
        };
      } else {
        return null;
      }
    }
  }

  async function getItem(pos: PositionInTimeline): Promise<TimelineItem> {
    const section = sections[pos.sectionIndex];
    if (section.segments === null) {
      await loadSection(pos.sectionIndex);
    }
    if (section.segments === null) {
      throw new Error('failed to load section');
    }
    return section.segments[pos.segmentIndex].items[pos.itemIndex];
  }

  async function hideSelectedAssets() {
    if (selectedItems.size === 0) {
      return;
    }
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    const assetIds = R.pipe(
      Array.from(selectedItems.values()),
      R.uniqueBy(({ item }) => (item.itemType === 'asset' ? item : item.series)),
      R.flatMap(({ item }) => (item.itemType === 'asset' ? [item] : item.series.assets)),
      R.map((asset) => asset.id),
    );
    await setAssetsHidden({ what: 'hide', assetIds });

    const untreatedItems = new Set(selectedItems.keys());
    const affectedSectionIdxs: number[] = [];
    for (let sectionIdx = 0; sectionIdx < sections.length; sectionIdx += 1) {
      const section = sections[sectionIdx];
      const segments = section.segments;
      if (!segments) {
        continue;
      }
      const segmentsToRemove: Set<number> = new Set();
      let newNumAssets = 0;
      for (let segmentIdx = 0; segmentIdx < segments.length; segmentIdx += 1) {
        if (untreatedItems.size === 0) {
          break;
        }
        const segment = segments[segmentIdx];
        const remainingItems: TimelineItem[] = [];
        for (const item of segment.items) {
          if (selectedItems.has(item.key)) {
            untreatedItems.delete(item.key);
          } else {
            remainingItems.push(item);
          }
        }
        if (
          remainingItems.length != segment.items.length &&
          ((affectedSectionIdxs.length > 0 && affectedSectionIdxs.at(-1) != sectionIdx) ||
            affectedSectionIdxs.length == 0)
        ) {
          affectedSectionIdxs.push(sectionIdx);
        }
        newNumAssets += R.pipe(
          remainingItems,
          // don't count split up series multiple times
          R.uniqueBy((it) => (it.itemType === 'asset' ? it : it.series)),
          R.map((it) => (it.itemType === 'asset' ? 1 : it.series.assets.length)),
          R.sum(),
        );
        if (remainingItems.length === 0) {
          segmentsToRemove.add(segmentIdx);
        } else {
          segment.items = remainingItems;
        }
      }
      if (segmentsToRemove.size > 0) {
        const remainingSegments = segments.filter((_s, idx) => !segmentsToRemove.has(idx));
        section.segments = remainingSegments;
        // a section could now be empty (no segments inside), but that doesn't really matter
        // since sections on their own are not displayed or anything
      }
      section.data.numAssets = newNumAssets;
    }

    let itemShiftAmount = 0;
    for (let i = 0; i < sections.length; i++) {
      const s = sections[i];
      if (s.items === null) {
        continue;
      }
      s.items.startIdx -= itemShiftAmount;
      s.items.endIdx -= itemShiftAmount;
      if (affectedSectionIdxs.indexOf(i) >= 0) {
        const numItems = s.items.endIdx - s.items.startIdx;
        itemShiftAmount += numItems;
        items.splice(s.items.startIdx, numItems);
        s.items = null;
      }
    }
    visibleItems.endIdx -= selectedItems.size; // not 100% sure on this
    selectedItems.clear();
    // reassign Items' asset index
    for (const sectionIdx of affectedSectionIdxs) {
      layoutSection(sectionIdx, 'noAdjustScroll');
    }
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
  }

  function setActualItemHeight(itemIndex: number, newHeight: number) {
    // remember probably header height so we don't have to guess next time
    const item = items[itemIndex];
    if (item.type === 'asset') {
      console.error('setActualItemHeight does not work for Items of type=asset');
      return;
    } else if (item.height === newHeight) {
      return;
    } else if (item.type === 'segmentTitle') {
      // here we assume that most segment titles are one line, and the smallest value of segmentTitle is probably one line height
      if (initialHeightGuess.segmentTitle === null || newHeight < initialHeightGuess.segmentTitle) {
        initialHeightGuess.segmentTitle = newHeight;
      }
    }
    const sectionIndex = sections.findIndex(
      (s) => s.items && s.items.startIdx <= itemIndex && itemIndex < s.items.endIdx,
    );
    if (sectionIndex < 0) {
      console.error('setActualItemHeight: did not find corresponding section');
      return;
    }
    if (sections[sectionIndex].items === null) {
      console.error('setActualItemHeight: sections[sectionIndex].items === null');
      return;
    }
    if (
      (item.type === 'segmentTitle' && item.titleType === 'major') ||
      item.type === 'createGroupTitleInput'
    ) {
      // find all minor titles with same row index, and set their height to this new height, shifting all items below
      const heightDelta = newHeight - item.height;
      if (heightDelta === 0) {
        return;
      }
      sections[sectionIndex].height += heightDelta;
      items[itemIndex].height += heightDelta;
      // items[i].top <= items[i+1].top, so shift starting from itemIndex onwards
      for (let i = itemIndex + 1; i < sections[sectionIndex].items.endIdx; i += 1) {
        items[i].top += heightDelta;
      }
      adjustScrollTop({
        what: 'scrollBy',
        scroll: heightDelta,
        ifScrollTopGt: item.top,
        behavior: 'instant',
      });
      for (let i = sectionIndex + 1; i < sections.length; i += 1) {
        const s = sections[i];
        s.top += heightDelta;
        if (s.items) {
          for (let j = s.items.startIdx; j < s.items.endIdx; j += 1) {
            items[j].top += heightDelta;
          }
        }
      }
    } else if (item.type === 'segmentTitle' && item.titleType === 'day') {
      const heightDelta = newHeight - item.height;
      if (heightDelta === 0) {
        return;
      }
      sections[sectionIndex].height += heightDelta;
      let firstTitleInRow: number = -1;
      for (let i = itemIndex; i >= 0; i -= 1) {
        const it = items[i];
        if (
          it.type === 'segmentTitle' &&
          (it.titleType === 'major' ||
            (it.titleType === 'day' && it.titleRowIndex !== item.titleRowIndex))
        ) {
          break;
        } else if (it.type === 'segmentTitle' && it.titleType === 'day') {
          firstTitleInRow = i;
        }
      }
      items[firstTitleInRow].height += heightDelta;
      // items[i].top <= items[i+1].top, so shift starting from itemIndex onwards
      for (let i = firstTitleInRow + 1; i < sections[sectionIndex].items.endIdx; i += 1) {
        const it = items[i];
        if (
          it.type === 'segmentTitle' &&
          it.titleType === 'day' &&
          it.titleRowIndex === item.titleRowIndex
        ) {
          // title in same row, adjust height
          it.height += heightDelta;
        } else {
          // other type of item, shift down
          items[i].top += heightDelta;
        }
      }
      adjustScrollTop({
        what: 'scrollBy',
        scroll: heightDelta,
        ifScrollTopGt: item.top,
        behavior: 'instant',
      });
      for (let i = sectionIndex + 1; i < sections.length; i += 1) {
        const s = sections[i];
        s.top += heightDelta;
        if (s.items) {
          for (let j = s.items.startIdx; j < s.items.endIdx; j += 1) {
            items[j].top += heightDelta;
          }
        }
      }
    }
  }

  async function getGridItemAtPosition(pos: PositionInTimeline): Promise<TimelineGridItem | null> {
    const section = sections[pos.sectionIndex];
    if (section.segments === null) {
      await loadSection(pos.sectionIndex);
    }
    if (section.segments === null) {
      throw new Error('error loading section');
    }
    layoutSection(pos.sectionIndex, 'noAdjustScroll');
    console.assert(section.items !== null);
    if (section.items === null) {
      return null;
    }

    // find item in items array
    let itemIndex = -1;
    for (let i = section.items.startIdx; i < section.items.endIdx; i += 1) {
      const item = items[i];
      if (item.type === 'asset' || item.type === 'photoStack') {
        if (
          item.timelineItem.pos.sectionIndex === pos.sectionIndex &&
          item.timelineItem.pos.segmentIndex === pos.segmentIndex &&
          item.timelineItem.pos.itemIndex === pos.itemIndex
        ) {
          itemIndex = i;
          break;
        }
      }
    }
    console.assert(itemIndex >= 0, 'loaded and laid out section but did not find correct item');
    if (itemIndex < 0) {
      return null;
    }
    return items[itemIndex];
  }

  async function createGroupClicked() {
    if (selectedItems.size === 0) {
      return;
    }
    const previousSections = klona(sections);
    const previousItems = klona(items);
    if (selectedItems.size === 0) {
      return;
    }
    const itemsInGroup: TimelineItem[] = [];
    const affectedSections: number[] = [];
    for (const [sectionIdx, section] of sections.entries()) {
      if (section.segments === null) {
        continue;
      }
      let thisSectionAffected = false;
      const newSegments: TimelineSegment[] = [];
      for (const segment of section.segments) {
        if (segment.type !== 'dateRange') {
          // TODO: add assets/move assets that are alread in group to other group
          newSegments.push(segment);
          continue;
        }
        // arrays of contiguous items, which may be separated by items in group
        const remainingItems: TimelineItem[][] = [];
        let currentlyInGroup = false;
        for (const item of segment.items) {
          if (selectedItems.has(item.key)) {
            currentlyInGroup = true;
            thisSectionAffected = true;
            itemsInGroup.push(item);
          } else {
            if (currentlyInGroup || remainingItems.length === 0) {
              currentlyInGroup = false;
              remainingItems.push([item]);
            } else {
              remainingItems.at(-1)!.push(item);
            }
          }
        }
        if (remainingItems.length === 1 && remainingItems[0].length > 0) {
          const startDate = (() => {
            const it = remainingItems[0][0];
            if (it.itemType === 'asset') {
              return it.takenDate;
            } else {
              return it.sortDate;
            }
          })();
          const endDate = (() => {
            const it = remainingItems[0].at(-1)!;
            if (it.itemType === 'asset') {
              return it.takenDate;
            } else {
              it.series.assets.at(-1)!.takenDate;
            }
          })();
          const newSegment: TimelineSegment = {
            type: 'dateRange',
            items: remainingItems[0],
            sortDate: startDate,
            itemRange: null,
            start: dayjs.utc(startDate),
            end: dayjs.utc(endDate),
          };
          newSegments.push(newSegment);
        } else {
          for (const items of remainingItems) {
            console.assert(items.length > 0);
            const startDate = (() => {
              const it = items[0];
              if (it.itemType === 'asset') {
                return it.takenDate;
              } else {
                return it.series.assets[0].takenDate;
              }
            })();
            const endDate = (() => {
              const it = items.at(-1)!;
              if (it.itemType === 'asset') {
                return it.takenDate;
              } else {
                it.series.assets.at(-1)!.takenDate;
              }
            })();
            const newSegment: TimelineSegment = {
              type: 'dateRange',
              items,
              sortDate: items[0].itemType === 'asset' ? items[0].takenDate : items[0].sortDate,
              itemRange: null,
              start: dayjs.utc(startDate),
              end: dayjs.utc(endDate),
            };
            newSegments.push(newSegment);
          }
        }
      }
      section.segments = newSegments;
      if (thisSectionAffected) {
        affectedSections.push(sectionIdx);
      }
    }
    const groupSortDate = itemsInGroup[0].sortDate; // most recent asset date in group
    if (!groupSortDate || affectedSections.length === 0) {
      return;
    }
    const insertInSectionIndex = affectedSections.findLast((i) => {
      if (!sections[i].segments || sections[i].segments.length === 0) {
        return false;
      }
      return sections[i].segments.at(-1)!.sortDate <= groupSortDate;
    });
    console.assert(insertInSectionIndex !== undefined && insertInSectionIndex >= 0);
    if (insertInSectionIndex === undefined || insertInSectionIndex < 0) {
      return;
    }
    console.assert(affectedSections.indexOf(insertInSectionIndex) >= 0);
    const section = sections[insertInSectionIndex];
    const insertBeforeSegmentIndex = section.segments!.findIndex(
      (s) => s.items.at(0)!.sortDate < groupSortDate,
    );
    const startDate = (() => {
      const it = itemsInGroup[0];
      if (it.itemType === 'asset') {
        return it.takenDate;
      } else {
        return it.series.assets[0].takenDate;
      }
    })();
    const endDate = (() => {
      const it = itemsInGroup.at(-1)!;
      if (it.itemType === 'asset') {
        return it.takenDate;
      } else {
        it.series.assets.at(-1)!.takenDate;
      }
    })();
    const newSegment: TimelineSegment & { type: 'creatingGroup' } = $state({
      type: 'creatingGroup',
      items: itemsInGroup,
      sortDate: groupSortDate,
      itemRange: null,
      start: dayjs.utc(startDate),
      end: dayjs.utc(endDate),
    });
    section.segments!.splice(insertBeforeSegmentIndex, 0, newSegment);
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    for (const i of affectedSections) {
      layoutSection(i, 'noAdjustScroll');
    }
    console.assert(newSegment.itemRange !== null);
    if (newSegment.itemRange !== null) {
      const scrollToItem = items[newSegment.itemRange.startIdx];
      adjustScrollTop({
        what: 'scrollTo',
        scroll: Math.max(0, scrollToItem.top - viewport.height / 2),
        ifScrollTopGt: 0,
        behavior: 'smooth',
      });
    }
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
    state = {
      state: 'creatingTimelineGroup',
      itemsInGroup: itemsInGroup,
      groupSortDate,
      previousItems,
      previousSections,
    };
  }

  async function cancelCreateGroup() {
    if (state.state !== 'creatingTimelineGroup') {
      return;
    }
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    sections = state.previousSections;
    items = state.previousItems;
    state = { state: 'justLooking' };
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
  }

  async function confirmCreateGroup(title: string): Promise<void> {
    if (state.state !== 'creatingTimelineGroup') {
      return;
    }
    clearSelection();
    const assetsInGroup = R.pipe(
      state.itemsInGroup,
      R.uniqueBy((it) => (it.itemType === 'asset' ? it : it.series)),
      R.flatMap((it) => (it.itemType === 'asset' ? [it] : it.series.assets)),
      R.map((asset) => asset.id),
    );
    const response = createTimelineGroupResponse.parse(
      (await createTimelineGroup({ name: title, assets: assetsInGroup })).data,
    );
    const { sectionIndex, segmentIndex } = (() => {
      for (let i = 0; i < sections.length; i += 1) {
        const segments = sections[i].segments;
        if (segments === null) {
          continue;
        }
        for (let j = 0; j < segments.length; j += 1) {
          if (segments[j].type === 'creatingGroup') {
            return { sectionIndex: i, segmentIndex: j };
          }
        }
      }
      return { sectionIndex: null, segmentIndex: null };
    })();
    if (sectionIndex === null || segmentIndex === null) {
      state = { state: 'justLooking' };
      console.error('state is creatingTimelineGroup but did not find creatingGroup segment');
      return;
    }
    const oldSegment = sections[sectionIndex].segments![segmentIndex];
    sections[sectionIndex].segments![segmentIndex] = {
      type: 'group' as const,
      items: oldSegment.items,
      sortDate: response.displayDate,
      itemRange: null,
      clickArea: null,
      groupId: response.timelineGroupId,
      title,
      start: oldSegment.start,
      end: oldSegment.end,
    };
    layoutSection(sectionIndex, 'adjustScroll');
    state = { state: 'justLooking' };
  }

  async function addSelectedToExistingGroup(groupId: string): Promise<void> {
    if (state.state !== 'creatingTimelineGroup') {
      return;
    }
    if (setAnimationsEnabled !== null) {
      await setAnimationsEnabled(true);
    }
    const affectedSections: number[] = [];
    let groupToAbsorb: (TimelineSegment & { type: 'creatingGroup' }) | null = null;
    let assetIdsInGroup: AssetId[] | null = null;
    const newSections = klona(sections);
    for (const [sectionIdx, section] of newSections.entries()) {
      if (section.segments === null) {
        continue;
      }
      const remainingSegments: TimelineSegment[] = [];
      for (const segment of section.segments) {
        if (segment.type === 'creatingGroup') {
          groupToAbsorb = segment;
          affectedSections.push(sectionIdx);
        } else {
          remainingSegments.push(segment);
        }
      }
      section.segments = remainingSegments;
      if (groupToAbsorb !== null) {
        // found it
        assetIdsInGroup = R.pipe(
          groupToAbsorb.items,
          R.uniqueBy((item) => (item.itemType === 'asset' ? item : item.series)),
          R.flatMap((item) =>
            item.itemType === 'asset' ? [item.id] : item.series.assets.map((a) => a.id),
          ),
        );
        section.data.numAssets -= assetIdsInGroup.length; // added back if they're going to be added to the same section
        break;
      }
    }
    console.assert(groupToAbsorb !== null && assetIdsInGroup !== null);
    if (groupToAbsorb === null || assetIdsInGroup === null) {
      return;
    }

    // await addToTimelineGroup({ assets: assetIdsInGroup, groupId });

    let mergeInto: (TimelineSegment & { type: 'group' }) | null = null;
    outer: for (const [sectionIdx, section] of newSections.entries()) {
      if (section.segments === null) {
        continue;
      }
      for (const segment of section.segments) {
        if (segment.type === 'group' && segment.groupId === groupId) {
          affectedSections.push(sectionIdx);
          section.data.numAssets += assetIdsInGroup.length;
          mergeInto = segment;
          break outer;
        }
      }
    }
    console.assert(mergeInto !== null);
    if (mergeInto === null) {
      return;
    }
    mergeInto.items.push(...groupToAbsorb.items);
    mergeInto.items.sort((a, b) => b.sortDate.localeCompare(a.sortDate));
    sections = newSections;
    layoutSection(affectedSections[0], 'noAdjustScroll');
    if (affectedSections[0] !== affectedSections[1]) {
      layoutSection(affectedSections[1], 'noAdjustScroll');
    }
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
    clearSelection();
    state = { state: 'justLooking' };
  }

  return {
    createGroupClicked,
    cancelCreateGroup,
    confirmCreateGroup,
    addSelectedToExistingGroup,
    get state() {
      return state.state;
    },
    get totalNumAssets() {
      return totalNumAssets;
    },
    get items() {
      return items;
    },
    get addToGroupClickAreas() {
      return addToGroupClickAreas;
    },
    get timelineHeight() {
      return timelineHeight;
    },
    get sections() {
      return sections;
    },
    get visibleItems() {
      return visibleItems;
    },
    get options() {
      return opts;
    },
    get numAssetsSelected() {
      return R.pipe(
        Array.from(selectedItems.values()),
        R.uniqueBy(({ item }) => (item.itemType === 'asset' ? item : item.series)),
        R.map(({ item }) => (item.itemType === 'asset' ? 1 : item.series.assets.length)),
        R.sum(),
      );
    },
    set setAnimationsEnabled(v: ((enabled: boolean) => Promise<void>) | null) {
      setAnimationsEnabled = v;
    },
    initialize,
    resize,
    onScrollChange,
    getGridItemAtPosition,
    setActualItemHeight,
    getNextItemPosition,
    getItem,
    setItemSelected,
    isItemSelected,
    clearSelection,
    hideSelectedAssets,
  };
}

function estimateHeight(
  section: ApiTimelineSection,
  lineWidth: number,
  targetRowHeight: number,
): number {
  if (lineWidth === 0) {
    return 0;
  }
  const unwrappedWidth = section.avgAspectRatio * section.numAssets * targetRowHeight * (7 / 10);
  const rows = Math.ceil(unwrappedWidth / (lineWidth * 0.3)); // avg line fill discount b/c we don't merge small segments yet
  const height = rows * targetRowHeight;

  return height;
}
