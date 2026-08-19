import type {
  AssetId,
  AssetWithSpe,
  TimelineSection as ApiTimelineSection,
  TimelineSegment as ApiTimelineSegment,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  TimelineItem as ApiTimelineItem,
  AssetSeriesId,
  TimelineGroupId,
  FileId,
  MirrorCorrection,
  TimelineMonthSlice,
} from '@api/myrti';
import { dayjs, type Dayjs } from '@lib/dayjs';
import { klona } from 'klona/json';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import { layoutSegments } from './layout';
import * as R from 'remeda';
import {
  createSeries,
  createTimelineGroup,
  getTimelineSections,
  getTimelineSegments,
  setAssetsHidden,
  setAssetIsSeriesSelection,
  editTimelineGroup,
  setAssetTransformCorrection,
} from '../../api/myrti';
import {
  createSeriesResponse,
  createTimelineGroupResponse,
  getTimelineSectionsResponse,
  getTimelineSegmentsResponse,
} from '../../api/myrti.zod';
import type {
  AddToGroupClickArea,
  ScrollbarMonth,
  TimelineBlock,
  TimelineGridItem,
  TimelineItem,
  TimelineSection,
  TimelineSegment,
} from './timeline-types';

export type AssetSeriesRef = { id: AssetSeriesId; assetIds: AssetId[]; selectionIndices: number[] };

/** Backlink from TimelineItem to its segment/section */
export type PositionInTimeline = {
  sectionIndex: number;
  segmentIndex: number;
  itemIndex: number;
};

export type Viewport = { width: number; height: number; scrollbarHeight: number };

export interface ITimelineGrid {
  readonly state: 'justLooking' | 'creatingTimelineGroup';
  readonly totalNumAssets: number;
  readonly sections: TimelineSection[];
  readonly visibleSections: number[];

  readonly options: TimelineOptions;
  readonly addToGroupClickAreas: AddToGroupClickArea[];
  readonly editGroupEnabled: 'none' | 'add' | 'remove';
  readonly createStackEnabled: boolean;
  readonly addToStackEnabled: boolean;
  readonly deleteStackEnabled: boolean;
  // /** Assets are highlighted when something is selected and shift is pressed to preview
  //  * possible range selection. */
  // readonly selectionPreviewIds: Map<AssetId, boolean>;

  initialize: (viewport: Viewport) => Promise<void>;
  resize: (viewport: Viewport, scrollTop: number) => void;
  set setAnimationsEnabled(v: ((enabled: boolean) => Promise<void>) | null);
  onScrollChange: (top: number) => void;
  getGridItemAtPosition: (pos: PositionInTimeline) => Promise<TimelineGridItem | null>;
  setActualBlockHeight: (sectionIdx: number, heights: number[][]) => void;
  getNextItemPosition: (
    pos: PositionInTimeline,
    dir: 'left' | 'right',
  ) => PositionInTimeline | null;
  getItem: (pos: PositionInTimeline) => Promise<TimelineItem>;
  getItemMustBeLoaded: (pos: PositionInTimeline) => TimelineItem;
  getItemForAsset: (assetId: AssetId) => TimelineItem;
  /** previous/newer item */
  clearSelection: () => void;
  setItemSelected: (item: TimelineItem, selected: boolean) => void;
  isItemSelected: (item: TimelineItem) => boolean;
  readonly numAssetsSelected: number;
  readonly selectedAssetIds: AssetId[];
  // /** @param clickedAssetIndex asset clicked to perform range selection */
  // setRangeSelected: (clickedAssetIndex: number, selected: boolean) => void;
  // /** Asset is hoevered while shift is pressed, selection range should be highlighted */
  // rangeSelectHover: (hoveredAssetIndex: number) => void;
  hideSelectedAssets: () => Promise<void>;

  createGroupClicked: () => Promise<void>;
  createStackClicked: () => Promise<void>;
  cancelCreateGroup: () => Promise<void>;
  confirmCreateGroup: (title: string) => Promise<void>;
  removeFromGroupClicked: () => Promise<void>;
  addSelectedToExistingGroup: (groupId: string) => Promise<void>;
  setAssetSeriesSelection: (assetId: string, isSeriesSelection: boolean) => Promise<void>;

  getAsset: (id: AssetId) => AssetWithSpe;
  getAssetSeries: (id: AssetSeriesId) => AssetSeriesRef;
  readonly scrollbarMonths: ScrollbarMonth[];
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
      sectionIndex: number;
      segmentIndex: number;
      itemsInGroup: TimelineItem[];
      groupSortDate: string;
      previousSections: TimelineSection[];
    };

type ScrollCallback = (params: {
  what: 'scrollBy' | 'scrollTo';
  scroll: number;
  behavior: 'smooth' | 'instant';
}) => void;

type MonthHeight = {
  year: number;
  /** 1 based index */
  month: number;
  heightInScrollbar: number;
  topInScrollbar: number;
  heightInTimeline: number;
  topInTimeline: number;
};

export function createTimeline(opts: TimelineOptions): ITimelineGrid {
  let adjustScrollTop: ScrollCallback | null = null;
  let isInitialized = false;
  let viewport: Viewport = { width: 0, height: 0, scrollbarHeight: 0 };
  let state: TimelineState = $state({ state: 'justLooking' } as TimelineState);
  let previousSections: TimelineSection[] = [];

  const assetsById = (() => {
    const obj: { [id: AssetId]: AssetWithSpe } = $state({});
    return {
      get: (id: AssetId): AssetWithSpe => {
        return obj[id];
      },
      set: (id: AssetId, asset: AssetWithSpe) => {
        obj[id] = asset;
      },
    };
  })();

  const assetSeriesById = (() => {
    const obj: { [id: AssetSeriesId]: AssetSeriesRef } = $state({});
    return {
      get: (id: AssetSeriesId): AssetSeriesRef => {
        return obj[id];
      },
      set: (id: AssetSeriesId, asset: AssetSeriesRef) => {
        obj[id] = asset;
      },
    };
  })();

  let sections: TimelineSection[] = [];
  let sectionsPublic: TimelineSection[] = $state([]);

  const sectionHeights: number[] = $derived(
    R.pipe(
      sectionsPublic,
      R.map((s) => s.heightEstimate),
      // R.map((s) =>
      //   s.blocks !== null ? R.sum(s.blocks.map((bl) => bl.fullHeight)) : s.heightEstimate,
      // ),
    ),
  );
  const sectionTops: number[] = $derived(
    R.pipe(
      sectionHeights,
      R.dropLast(1),
      R.reduce(
        (acc: number[], height) => {
          acc.push(acc[acc.length - 1] + height);
          return acc;
        },
        [0],
      ),
    ),
  );

  const timelineHeight: number = $derived(R.sum(sectionHeights));
  const addToGroupClickAreas: AddToGroupClickArea[] = $derived(
    state.state === 'creatingTimelineGroup'
      ? R.pipe(
          sections,
          R.flatMap((s) => s.segments ?? []),
          R.filter((seg) => seg.type === 'group'),
          R.map((seg) => seg.clickArea),
          R.filter((area) => area !== null),
        )
      : [],
  );
  let sectionsInView: number[] = $state([]);
  // createView segment always has to be visible so we can scroll to it
  const visibleSections = $derived(
    R.unique(
      sectionsInView.concat(state.state === 'creatingTimelineGroup' ? [state.sectionIndex] : []),
    ),
  );
  let setAnimationsEnabled: ((enabled: boolean) => Promise<void>) | null = null;
  /** maps item key string to index in selection */
  const selectedItems: Map<string, { item: TimelineItem; idx: number }> = new SvelteMap();
  const totalNumAssets: number = $derived(
    sections.reduce((acc: number, section) => {
      return acc + section.data.numAssets;
    }, 0),
  );

  // eslint-disable-next-line svelte/prefer-svelte-reactivity
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

  let sectionMonthSlices: TimelineMonthSlice[][] = [];
  let scrollbarMonths: ScrollbarMonth[] = $state([]);
  let scrollbarMonthHeights: { year: number; month: number; height: number; sectionIdx: number }[] =
    [];
  let scrollbarY = $state(0);
  let scrollbarYMax = $derived(
    R.pipe(
      scrollbarMonths,
      R.map((m) => m.height),
      R.sum(),
    ),
  );

  function computeScrollbarMonths() {
    scrollbarMonthHeights = sectionMonthSlices.flatMap((sectionMonths, sectionIdx) => {
      return sectionMonths.flatMap(({ year, month, totalNormalizedWidth }) => {
        let height;
        if (sectionsPublic[sectionIdx].blocks) {
          height = R.pipe(
            sectionsPublic[sectionIdx].blocks,
            R.filter((block) => {
              return block.sortDate.year() === year && block.sortDate.month() + 1 === month;
            }),
            R.map((block) => block.fullHeight),
            R.sum(),
          );
        } else {
          height = Math.max(
            opts.targetRowHeight,
            estimateHeight(totalNormalizedWidth, viewport.width, opts.targetRowHeight),
          );
        }
        return {
          sectionIdx,
          month,
          year,
          height,
        };
      });
    });
    {
      const totalHeight = R.pipe(
        scrollbarMonthHeights,
        R.map((m) => m.height),
        R.sum(),
      );
      for (const m of scrollbarMonthHeights) {
        m.height = (m.height / totalHeight) * viewport.scrollbarHeight;
      }
    }

    let cumulHeight = 0;
    const monthMarkers: ScrollbarMonth[] = [];
    let lastMarkerTop = 0;
    for (const { year, month, height } of scrollbarMonthHeights) {
      const showYear =
        monthMarkers.length === 0 || year !== monthMarkers[monthMarkers.length - 1].year;
      const showMonth =
        monthMarkers.length === 0 ||
        (month !== monthMarkers[monthMarkers.length - 1].month && cumulHeight - lastMarkerTop > 6);
      if (showMonth) {
        lastMarkerTop = cumulHeight;
      }
      monthMarkers.push({
        year,
        month,
        height,
        showYear,
        showMonth,
        top: cumulHeight,
      });
      cumulHeight += height;
    }
    const yearMarkers = monthMarkers.filter((month) => month.showYear);
    const yearStarts = yearMarkers.map((m) => m.top);
    const keepYears = selectYearLabels(yearStarts, 8);
    for (const [i, marker] of yearMarkers.entries()) {
      marker.showYear = keepYears.has(i);
    }
    for (const m of monthMarkers) {
      m.height = (m.height / cumulHeight) * viewport.scrollbarHeight;
      m.top = (m.top / cumulHeight) * viewport.scrollbarHeight;
    }
    scrollbarMonths = monthMarkers;
  }

  async function loadSectionPlaceholders() {
    const { sections: sectionData, monthsSummary } = (await getTimelineSections()).data;

    sectionMonthSlices = monthsSummary;
    sections = sectionData.map((section) => {
      return {
        data: section,
        heightEstimate: estimateHeight(
          section.totalNormalizedWidth,
          viewport.width,
          opts.targetRowHeight,
        ),
        segments: null,
        startDate: dayjs.utc(section.startDate),
        endDate: dayjs.utc(section.endDate),
        blocks: null,
      };
    });
    sectionsPublic = sections;
    computeScrollbarMonths();
  }

  const monthHeights: MonthHeight[] = $derived.by(() => {
    const months: MonthHeight[] = [];
    const pushOrAdd = (year: number, month: number, heightInTimeline: number) => {
      if (months.length > 0) {
        const last = months[months.length - 1];
        if (last.year === year && last.month === month) {
          last.heightInTimeline += heightInTimeline;
          return;
        }
      }
      months.push({
        year,
        month,
        heightInTimeline,
        topInTimeline: 0,
        heightInScrollbar: 0,
        topInScrollbar: 0,
      });
    };
    for (let sectionIdx = 0; sectionIdx < sectionsPublic.length; sectionIdx += 1) {
      const section = sectionsPublic[sectionIdx];
      if (section.blocks !== null) {
        for (const block of section.blocks) {
          const date = block.sortDate.tz('utc');
          pushOrAdd(date.year(), date.month() + 1, block.fullHeight); // dayjs months are 0 indexed
        }
      } else {
        for (const { year, month, totalNormalizedWidth } of sectionMonthSlices[sectionIdx]) {
          const height = Math.max(
            opts.targetRowHeight,
            estimateHeight(totalNormalizedWidth, viewport.width, opts.targetRowHeight),
          );
          pushOrAdd(year, month, height);
        }
      }
    }
    let top = 0;
    for (const month of months) {
      month.topInTimeline = top;
      top += month.heightInTimeline;
    }

    console.assert(months.length === scrollbarMonths.length);
    let scrollBarTop = 0;
    for (let i = 0; i < months.length; i++) {
      const m = months[i];
      const sm = scrollbarMonths[i];
      console.assert(m.year === sm.year && m.month === sm.month);
      m.topInScrollbar = scrollBarTop;
      m.heightInScrollbar = sm.height;
      scrollBarTop += sm.height;
    }
    return months;
  });

  function resize(newViewport: Viewport, scrollTop: number) {
    if (viewport === newViewport) {
      return;
    }
    const forceRelayout = viewport.width !== newViewport.width;
    viewport = { ...newViewport };
    onSectionIntersectChanged(scrollTop, forceRelayout);
    computeScrollbarMonths();
  }

  async function onSectionIntersectChanged(scrollTop: number, forceRelayout: boolean = false) {
    const renderWithinMargin = opts.loadWithinMargin;
    const loadWithinMargin = 3000;
    let firstLoadedSection = null;
    let lastLoadedSection = null;
    const visible: number[] = [];
    for (let i = 0; i < sections.length; i += 1) {
      const sectionTop = sectionTops[i];
      const sectionHeight = sectionHeights[i];
      const isVisible =
        sectionTop <= scrollTop + viewport.height + renderWithinMargin &&
        scrollTop - renderWithinMargin <= sectionTop + sectionHeight;
      if (isVisible) {
        visible.push(i);
      }

      const isLoaded =
        sectionTop <= scrollTop + viewport.height + loadWithinMargin &&
        scrollTop - loadWithinMargin <= sectionTop + sectionHeight;
      if (firstLoadedSection == null && isLoaded) {
        firstLoadedSection = i;
        lastLoadedSection = i;
      } else if (isLoaded) {
        lastLoadedSection = i;
      }
    }
    if (visible.length === 0 || firstLoadedSection === null || lastLoadedSection === null) {
      return;
    }

    const sectionLoads = [];
    for (let i = firstLoadedSection; i <= lastLoadedSection; i += 1) {
      if (sections[i].blocks === null) {
        sectionLoads.push(loadSection(i));
        sections[i].isLoading = true;
      }
    }
    sectionsPublic = sections;
    if (sectionLoads.length > 0) {
      const now = Date.now();
      lastScrollTime = now;
      await Promise.all(sectionLoads);
      if (lastScrollTime != now) {
        return;
      }
      // after waiting for all sections to load. We could do this progressively after any one section loads, but that makes things more complicated for little reason
      // TODO: make the above irrelevant by adding an API call to load multiple sections at once, for the rare event that the user jumps exactly inbetween two sections.
      for (let i = firstLoadedSection; i <= lastLoadedSection; i += 1) {
        if (sections[i].blocks === null || forceRelayout) {
          layoutSection(i);
        }
      }
    }
    sectionsInView = visible;
    if (sectionLoads.length > 0 || forceRelayout) {
      sectionsPublic = sections;
    }
    onScrollChange(scrollTop);
  }
  $inspect(visibleSections);

  let currentScrollTop = 0;
  let lastScrollTime: number | null = null;
  async function onScrollChange(scrollTop: number) {
    currentScrollTop = scrollTop;
    const firstVisibleMonth = monthHeights.find(
      (m) =>
        m.topInTimeline < scrollTop + viewport.height &&
        scrollTop < m.topInTimeline + m.heightInTimeline,
    );
    if (firstVisibleMonth) {
      const firstMonthProgress =
        (scrollTop - firstVisibleMonth.topInTimeline) / firstVisibleMonth.heightInTimeline;
      console.assert(
        -1e-3 < firstMonthProgress && firstMonthProgress < 1 + 1e-3,
        firstMonthProgress,
      );
      const clampedProgress = Math.max(Math.min(firstMonthProgress, 1.0), 0.0);
      let scrollY = 0;
      let foundMonth = false; // scrollbarMonthHeights keeps months split if they span multiple sections
      let currentMonthHeight = 0;
      for (const { year, month, height } of scrollbarMonthHeights) {
        const monthEqual = year === firstVisibleMonth.year && month === firstVisibleMonth.month;
        if (monthEqual) {
          currentMonthHeight += height;
          foundMonth = true;
        } else if (foundMonth && !monthEqual) {
          scrollY += currentMonthHeight * clampedProgress;
          break;
        } else {
          scrollY += height;
        }
      }
      scrollbarY = scrollY;
    }
  }

  function layoutSection(sectionIndex: number, adjustScroll = 'adjustScroll') {
    const section = sections[sectionIndex];
    const segments = section.segments;
    if (segments === null) {
      console.error('sections[sectionIndex].segments must not be null in layoutSection()');
      return;
    }
    const lastSectionEndDate = sectionIndex === 0 ? null : sections[sectionIndex - 1].endDate;
    const { blocks } = layoutSegments(
      segments,
      lastSectionEndDate,
      viewport.width,
      opts,
      assetsById.get,
      assetSeriesById.get,
    );
    section.blocks = blocks;
    // Scroll adjust will trigger again once DOM elements are measured.
    // Could be done smarter to combine the two updates.
    const oldHeight = section.heightEstimate;
    section.heightEstimate = R.sum(blocks.map((b) => b.fullHeight));
    const delta = section.heightEstimate - oldHeight;
    if (adjustScroll === 'adjustScroll') {
      adjustScrollTop?.({
        what: 'scrollBy',
        scroll: delta,
        behavior: 'instant',
      });
    }
  }

  async function loadSection(sectionIndex: number, reload: 'reload' | undefined = undefined) {
    const section = sections[sectionIndex];
    if (section.blocks && reload === undefined) {
      return;
    }

    const sectionId = section.data.id;
    const segments = await requestSegments(sectionId);

    for (const segment of segments) {
      for (const item of segment.items) {
        if (item.itemType === 'asset') {
          assetsById.set(item.assetId, item);
        } else {
          for (const asset of item.assets) {
            assetsById.set(asset.assetId, asset);
          }
          assetSeriesById.set(item.seriesId, {
            id: item.seriesId,
            assetIds: item.assets.map((a) => a.assetId),
            selectionIndices: item.selectionIndices,
          });
        }
      }
    }
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
              itemType: 'asset',
              assetId: item.assetId,
              key: `a-${item.assetId}`,
              sortDate: item.takenDate,
              pos: { sectionIndex, segmentIndex, itemIndex },
            });
            itemIndex += 1;
          } else {
            // item.itemType === 'photoSeries'
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
                key: `s-${item.seriesId}-${splitStart}-${splitEnd}`,
                itemType: 'photoStack',
                coverIndex: selectionIdx,
                seriesId: item.seriesId,
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
            start: dayjs.utc(segment.start),
            end: dayjs.utc(segment.end),
            gridItems: null,
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
            clickArea: null,
            start: dayjs.utc(startDate),
            end: dayjs.utc(endDate),
            gridItems: [],
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
  function setItemSelected(item: TimelineGridItem, selected: boolean) {
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
        if (it.itemType === 'photoStack' && it.seriesId === item.seriesId) {
          itemsOfSameSeries.push(it);
        } else {
          break;
        }
      }
      for (let i = item.pos.itemIndex - 1; 0 <= i; i -= 1) {
        const it = segment.items[i];
        if (it.itemType === 'photoStack' && it.seriesId === item.seriesId) {
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
    // console.log('getNextItemPosition', $state.snapshot(pos), dir);
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
        const prevSection = sections[pos.sectionIndex - 1];
        const segs = prevSection.segments;
        if (segs === null) {
          console.error('timeline getNextItemPosition: previous section is not loaded');
          return null;
        }
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

  function getItemForAsset(assetId: AssetId): TimelineItem {
    for (const section of sections) {
      if (section.segments !== null) {
        for (const segment of section.segments) {
          for (const item of segment.items) {
            if (item.itemType === 'asset' && item.assetId === assetId) {
              return item;
            } else if (item.itemType === 'photoStack') {
              const series = assetSeriesById.get(item.seriesId);
              if (
                series &&
                series.assetIds.find(
                  (id, i) => item.splitStart <= i && i < item.splitEnd && id === assetId,
                )
              ) {
                return item;
              }
            }
          }
        }
      }
    }
    return null;
    throw new Error('TODO did not find item in loaded section');
  }

  function getItemMustBeLoaded(pos: PositionInTimeline): TimelineItem {
    const section = sections[pos.sectionIndex];
    if (section.segments === null) {
      throw new Error('TODO');
    }
    return section.segments[pos.segmentIndex].items[pos.itemIndex];
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
      R.map((asset) => asset.assetId),
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
          R.uniqueBy((it) => (it.itemType === 'asset' ? it : it.seriesId)),
          R.map((it) =>
            it.itemType === 'asset' ? 1 : assetSeriesById.get(it.seriesId).assetIds.length,
          ),
          R.sum(),
        );
        if (remainingItems.length === 0) {
          segmentsToRemove.add(segmentIdx);
        } else {
          segment.items = remainingItems;
          for (const [idx, item] of segment.items.entries()) {
            item.pos.itemIndex = idx;
          }
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
    selectedItems.clear();
    // reassign Items' asset index
    for (const sectionIdx of affectedSectionIdxs) {
      layoutSection(sectionIdx);
    }
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
    sectionsPublic = sections;
  }

  function setActualBlockHeight(sectionIdx: number, heights: number[][]) {
    let totalDelta = 0;
    let scrollAdjustDelta = 0;
    let anyChange = false;
    const section = sections[sectionIdx];
    let scrollToBlock: TimelineBlock | null = null;
    for (const [blockIdx, height] of heights) {
      const block = section.blocks?.at(blockIdx);
      if (R.isNonNullish(block)) {
        const delta = height - block.fullHeight;
        totalDelta += delta;
        if (!block.hasBeenMeasured && block.blockType === 'createGroup') {
          scrollToBlock = block;
        }
        if (!block.hasBeenMeasured && block.top + block.fullHeight < currentScrollTop) {
          // Scroll only needs to be adjusted when switching from estimated to measured height.
          // If block has already been laid out and changes we don't want to scroll.
          scrollAdjustDelta += delta;
        }
        block.fullHeight = height;
        block.hasBeenMeasured = true;
        if (delta !== 0) {
          anyChange = true;
        }
      } else {
        console.error('block is null but measured its height somehow');
      }
    }
    if (!anyChange) {
      return;
    }
    let blockTop = 0;
    for (const block of section.blocks) {
      block.top = blockTop;
      blockTop += block.fullHeight;
    }

    if (totalDelta !== 0) {
      sections[sectionIdx].heightEstimate += totalDelta;
    }
    sectionsPublic = sections;
    if (scrollToBlock !== null) {
      adjustScrollTop?.({
        what: 'scrollTo',
        scroll: Math.max(0, sectionTops[sectionIdx] + scrollToBlock.top - viewport.height * 0.25),
        behavior: 'smooth',
      });
      return;
    } else if (scrollAdjustDelta !== 0) {
      adjustScrollTop?.({
        what: 'scrollBy',
        scroll: scrollAdjustDelta,
        behavior: 'instant',
      });
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
    // FIXME: why was this called here. it mutates $state which can't happen anymore in a getter
    // layoutSection(pos.sectionIndex, 'noAdjustScroll');
    console.assert(section.items !== null);
    if (section.items === null) {
      return null;
    }

    // find item in items array
    for (let i = section.items.startIdx; i < section.items.endIdx; i += 1) {
      const item = items[i];
      if (item.type === 'asset' || item.type === 'photoStack') {
        if (
          item.timelineItem.pos.sectionIndex === pos.sectionIndex &&
          item.timelineItem.pos.segmentIndex === pos.segmentIndex &&
          item.timelineItem.pos.itemIndex === pos.itemIndex
        ) {
          return items[i];
        }
      }
    }
    console.error('loaded and laid out section but did not find correct item');
    return null;
  }

  async function createGroupClicked() {
    if (selectedItems.size === 0) {
      return;
    }
    previousSections = klona(sections);
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
        // FIXME: this logic is wrong. the only segment that can get split is the one where group's sort date lands inside of
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
              return assetsById.get(it.assetId).takenDate;
            } else {
              const lastAssetId = assetSeriesById.get(it.seriesId).assetIds.at(-1);
              return assetsById.get(lastAssetId).takenDate;
            }
          })();
          const endDate = (() => {
            const it = remainingItems[0].at(-1)!;
            if (it.itemType === 'asset') {
              return assetsById.get(it.assetId).takenDate;
            } else {
              const lastAssetId = assetSeriesById.get(it.seriesId).assetIds.at(-1);
              return assetsById.get(lastAssetId).takenDate;
            }
          })();
          const newSegment: TimelineSegment = {
            type: 'dateRange',
            items: remainingItems[0],
            sortDate: startDate,
            start: dayjs.utc(startDate),
            end: dayjs.utc(endDate),
          };
          newSegments.push(newSegment);
        } else {
          for (const items of remainingItems) {
            const startDate = (() => {
              const it = items[0];
              if (it.itemType === 'asset') {
                return assetsById.get(it.assetId).takenDate;
              } else {
                const firstAssetId = assetSeriesById.get(it.seriesId).assetIds[0];
                return assetsById.get(firstAssetId).takenDate;
              }
            })();
            const endDate = (() => {
              const it = items.at(-1)!;
              if (it.itemType === 'asset') {
                return assetsById.get(it.assetId).takenDate;
              } else {
                const lastAssetId = assetSeriesById.get(it.seriesId).assetIds.at(-1);
                return assetsById.get(lastAssetId).takenDate;
              }
            })();
            const newSegment: TimelineSegment = {
              type: 'dateRange',
              items,
              sortDate:
                items[0].itemType === 'asset'
                  ? assetsById.get(items[0].assetId).takenDate
                  : items[0].sortDate,
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
        return assetsById.get(it.assetId).takenDate;
      } else {
        const firstAssetId = assetSeriesById.get(it.seriesId).assetIds[0];
        return assetsById.get(firstAssetId).takenDate;
      }
    })();
    const endDate = (() => {
      const it = itemsInGroup.at(-1)!;
      if (it.itemType === 'asset') {
        return assetsById.get(it.assetId).takenDate;
      } else {
        const lastAssetId = assetSeriesById.get(it.seriesId).assetIds.at(-1);
        return assetsById.get(lastAssetId).takenDate;
      }
    })();
    const newSegment: TimelineSegment & { type: 'creatingGroup' } = {
      type: 'creatingGroup',
      items: itemsInGroup,
      sortDate: groupSortDate,
      start: dayjs.utc(startDate),
      end: dayjs.utc(endDate),
    };
    section.segments!.splice(insertBeforeSegmentIndex, 0, newSegment);
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    for (const i of affectedSections) {
      layoutSection(i, 'noAdjustScroll');
      if (!sectionsInView.includes(i)) {
        sectionsInView.push(i);
      }
    }
    for (let sectionIdx = 0; sectionIdx < sections.length; sectionIdx += 1) {
      const section = sections[sectionIdx];
      if (section.segments === null) {
        continue;
      }
      for (let segmentIdx = 0; segmentIdx < section.segments.length; segmentIdx += 1) {
        const segment = section.segments[segmentIdx];
        for (let itemIdx = 0; itemIdx < segment.items.length; itemIdx += 1) {
          segment.items[itemIdx].pos = {
            sectionIndex: sectionIdx,
            segmentIndex: segmentIdx,
            itemIndex: itemIdx,
          };
        }
      }
    }
    state = {
      sectionIndex: insertInSectionIndex,
      segmentIndex: insertBeforeSegmentIndex,
      state: 'creatingTimelineGroup',
      itemsInGroup: itemsInGroup,
      groupSortDate,
    };
    sectionsPublic = sections;
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
  }

  async function cancelCreateGroup() {
    if (state.state !== 'creatingTimelineGroup') {
      return;
    }
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    sections = previousSections;
    state = { state: 'justLooking' };
    sectionsPublic = sections;
    previousSections = [];
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
  }

  async function confirmCreateGroup(title: string): Promise<void> {
    if (state.state !== 'creatingTimelineGroup') {
      return;
    }
    const assetsInGroup = R.pipe(
      state.itemsInGroup,
      R.uniqueBy((it) => (it.itemType === 'asset' ? it : it.seriesId)),
      R.flatMap((it) =>
        it.itemType === 'asset' ? [it.assetId] : assetSeriesById.get(it.seriesId).assetIds,
      ),
    );
    const { sectionIndex, segmentIndex } = state;
    const response = createTimelineGroupResponse.parse(
      (await createTimelineGroup({ name: title, assets: assetsInGroup })).data,
    );
    clearSelection();
    const oldSegment = sections[sectionIndex].segments![segmentIndex];
    if (oldSegment.type !== 'creatingGroup') {
      state = { state: 'justLooking' };
      console.error('state is creatingTimelineGroup but did not find creatingGroup segment');
      return;
    }
    sections[sectionIndex].segments![segmentIndex] = {
      type: 'group' as const,
      items: oldSegment.items,
      sortDate: response.displayDate,
      clickArea: null,
      groupId: response.timelineGroupId,
      title,
      start: oldSegment.start,
      end: oldSegment.end,
    };
    layoutSection(sectionIndex);
    state = { state: 'justLooking' };
    sectionsPublic = sections;
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
    for (const [sectionIdx, section] of sections.entries()) {
      if (section.segments === null) {
        continue;
      }
      const remainingSegments: TimelineSegment[] = [];
      for (const segment of section.segments) {
        if (segment.type === 'creatingGroup') {
          groupToAbsorb = segment;
          if (affectedSections.indexOf(sectionIdx) < 0) {
            affectedSections.push(sectionIdx);
          }
        } else {
          remainingSegments.push(segment);
        }
      }
      section.segments = remainingSegments;
      if (groupToAbsorb !== null) {
        // found it
        assetIdsInGroup = R.pipe(
          groupToAbsorb.items,
          R.uniqueBy((item) => (item.itemType === 'asset' ? item : item.seriesId)),
          R.flatMap((item) =>
            item.itemType === 'asset'
              ? [item.assetId]
              : assetSeriesById.get(item.seriesId)?.assetIds,
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

    await editTimelineGroup({ assets: assetIdsInGroup, groupId, operation: 'add' });

    let mergeInto: (TimelineSegment & { type: 'group' }) | null = null;
    outer: for (const [sectionIdx, section] of sections.entries()) {
      if (section.segments === null) {
        continue;
      }
      for (const segment of section.segments) {
        if (segment.type === 'group' && segment.groupId === groupId) {
          if (affectedSections.indexOf(sectionIdx) < 0) {
            affectedSections.push(sectionIdx);
          }
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
    for (const sectionIndex of affectedSections) {
      layoutSection(sectionIndex);
      if (!sections[sectionIndex].segments) {
        continue;
      }
      // reassign item positions
      for (const [segmentIndex, segment] of sections[sectionIndex].segments.entries()) {
        for (const [itemIndex, item] of segment.items.entries()) {
          item.pos = { sectionIndex, segmentIndex, itemIndex };
        }
      }
    }
    sectionsPublic = sections;
    if (setAnimationsEnabled) {
      setAnimationsEnabled(false);
    }
    clearSelection();
    state = { state: 'justLooking' };
  }

  async function createStackClicked() {
    if (selectedItems.size === 0) {
      return;
    }

    for (const item of selectedItems.values()) {
      // TODO: should probably just merge stacks?
      console.assert(item.item.itemType === 'asset');
      if (item.item.itemType !== 'asset') {
        return;
      }
    }
    const assetIds = R.pipe(
      selectedItems.values().toArray(),
      R.flatMap((item) => (item.item.itemType === 'asset' ? [item.item.assetId] : [])),
    );
    createSeriesResponse.parse((await createSeries({ assetIds })).data);
    clearSelection();
  }

  async function setAssetSeriesSelection(assetId: string, isSeriesSelection: boolean) {
    const response = await setAssetIsSeriesSelection(assetId, { isSeriesSelection });
    const newSeries = response.data;
    for (const [sectionIdx, section] of sections.entries()) {
      if (section.segments !== null) {
        outer: for (const segment of section.segments) {
          for (const item of segment.items) {
            if (item.itemType === 'photoStack' && item.seriesId === newSeries.seriesId) {
              if (!R.isDeepEqual(newSeries.assetIds, assetSeriesById.get(item.seriesId).assetIds)) {
                console.error('TODO: asset series/stack changed, not handled yet');
                return;
              }
              await loadSection(sectionIdx, 'reload');
              layoutSection(sectionIdx);
              break outer;
            }
          }
        }
      }
    }
  }

  // TODO: exit creating group mode when selection is canceled or empty
  $effect(() => {
    if (state.state === 'creatingTimelineGroup' && selectedItems.size === 0) {
      cancelCreateGroup();
    }
  });

  const editGroupEnabled = $derived.by(() => {
    if (state.state !== 'justLooking') {
      return 'none';
    }
    if (selectedItems.size === 0) {
      return 'none';
    }
    let selectedGroupId = null;
    let anyNotInGroup = false;
    let anyInGroup = false;
    for (const { item } of selectedItems.values()) {
      const { segment } = getContainingSegment(item);
      if (!segment) {
        console.error('containing segment is null');
        return 'none';
      }
      if (segment.type !== 'group') {
        anyNotInGroup = true;
      } else if (selectedGroupId === null) {
        selectedGroupId = segment.groupId;
        anyInGroup = true;
      } else if (segment.groupId !== selectedGroupId) {
        return 'none';
      }
    }
    if (!anyInGroup) {
      return 'add';
    } else if (anyInGroup && !anyNotInGroup) {
      return 'remove';
    }
    return 'none';
  });

  const createStackEnabled = $derived.by(() => {
    if (state.state !== 'justLooking') {
      return false;
    }
    if (
      selectedItems.size < 2 ||
      selectedItems.values().find((item) => item.item.itemType === 'photoStack') !== undefined
    ) {
      return false;
    }
    let anyInGroup = null;
    for (const { item } of selectedItems.values()) {
      console.assert(item.itemType === 'asset');
      if (item.itemType === 'asset') {
        const groupId = getContainingGroup(item);
        if ((groupId === null) !== (anyInGroup === null)) {
          return false;
        } else if (groupId !== null && anyInGroup !== null && groupId !== anyInGroup) {
          return false;
        }
        anyInGroup = groupId;
      }
    }
    return true;
  });

  const addToStackEnabled = $derived.by(() => {
    if (state.state !== 'justLooking') {
      return false;
    }
    if (selectedItems.size < 2) {
      return false;
    }
    let seriesId = null;
    for (const { item } of selectedItems.values()) {
      if (item.itemType === 'photoStack') {
        if (seriesId !== null && seriesId !== item.seriesId) {
          return false;
        }
        seriesId = item.seriesId;
      }
    }
    return seriesId !== null;
  });

  const deleteStackEnabled = $derived.by(() => {
    if (state.state !== 'justLooking') {
      return false;
    }
    let seriesId = null;
    for (const { item } of selectedItems.values()) {
      if (item.itemType === 'photoStack') {
        seriesId = item.seriesId;
      } else {
        return false;
      }
    }
    return seriesId !== null;
  });

  const removeFromGroupClicked = async () => {
    if (selectedItems.size === 0) {
      return;
    }

    const assetIds: AssetId[] = [];
    let groupId = null;
    const affectedSections = [];
    for (const item of selectedItems.values()) {
      console.assert(item.item.itemType === 'asset');
      if (item.item.itemType !== 'asset') {
        return;
      }
      const groupId_ = getContainingGroup(item.item);
      if (groupId !== null && groupId !== groupId_) {
        console.error('items from multiple groups selected');
        return;
      }
      groupId = groupId_;
      assetIds.push(item.item.assetId);
      const sectionIdx = item.item.pos.sectionIndex;
      if (affectedSections.indexOf(sectionIdx) < 0) {
        affectedSections.push(sectionIdx);
      }
    }
    if (groupId === null) {
      console.error('not items in group selected');
      return;
    }
    await editTimelineGroup({ groupId, assets: assetIds, operation: 'remove' });
    clearSelection();
    for (const sectionIdx of affectedSections) {
      await loadSection(sectionIdx, 'reload');
      layoutSection(sectionIdx);
    }
  };

  function getContainingSegment(item: TimelineItem): { segment: TimelineSegment | null } {
    return { segment: sections[item.pos.sectionIndex].segments[item.pos.segmentIndex] };
  }

  function getContainingGroup(item: TimelineItem & { itemType: 'asset' }): TimelineGroupId | null {
    const { segment } = getContainingSegment(item);
    if (!segment || segment.type !== 'group') {
      return null;
    }
    return segment.groupId;
  }

  return {
    createGroupClicked,
    cancelCreateGroup,
    confirmCreateGroup,
    removeFromGroupClicked,
    addSelectedToExistingGroup,
    createStackClicked,
    setAssetSeriesSelection,
    get state() {
      return state.state;
    },
    get totalNumAssets() {
      return totalNumAssets;
    },
    get addToGroupClickAreas() {
      return addToGroupClickAreas;
    },
    get timelineHeight() {
      return timelineHeight;
    },
    get sections() {
      return sectionsPublic;
    },
    get visibleSections() {
      return visibleSections;
    },
    get options() {
      return opts;
    },
    get numAssetsSelected() {
      return R.pipe(
        Array.from(selectedItems.values()),
        R.uniqueBy(({ item }) => (item.itemType === 'asset' ? item : item.seriesId)),
        R.map(({ item }) =>
          item.itemType === 'asset' ? 1 : assetSeriesById.get(item.seriesId).assetIds.length,
        ),
        R.sum(),
      );
    },
    get selectedAssetIds() {
      return R.pipe(
        Array.from(selectedItems.values()),
        R.uniqueBy(({ item }) => (item.itemType === 'asset' ? item : item.seriesId)),
        R.flatMap(({ item }) =>
          item.itemType === 'asset' ? [item.assetId] : assetSeriesById.get(item.seriesId).assetIds,
        ),
      );
    },
    get editGroupEnabled() {
      return editGroupEnabled;
    },
    get createStackEnabled() {
      return createStackEnabled;
    },
    get addToStackEnabled() {
      return addToStackEnabled;
    },
    get deleteStackEnabled() {
      return deleteStackEnabled;
    },
    set setAnimationsEnabled(v: ((enabled: boolean) => Promise<void>) | null) {
      setAnimationsEnabled = v;
    },
    getAssetSeries: (id) => assetSeriesById.get(id),
    getAsset: (id) => assetsById.get(id),
    initialize,
    resize,
    onScrollChange,
    onSectionIntersectChanged,
    getGridItemAtPosition,
    setActualBlockHeight,
    getNextItemPosition,
    getItem,
    getItemMustBeLoaded,
    getItemForAsset,
    setItemSelected,
    isItemSelected,
    clearSelection,
    hideSelectedAssets,
    get monthHeights() {
      return monthHeights;
    },
    monthForScrollY: (scrollY: number) => {
      return monthHeights.find(
        (m) => scrollY * timelineHeight < m.topInTimeline + m.heightInTimeline,
      );
    },
    get sectionTops() {
      return sectionTops;
    },
    get sectionHeights() {
      return sectionHeights;
    },
    get scrollbarY() {
      return scrollbarY;
    },
    get scrollbarYMax() {
      return scrollbarYMax;
    },
    set adjustScrollTop(cb: ScrollCallback | null) {
      adjustScrollTop = cb;
    },
    rotateAssetCW: async (assetId: AssetId) => {
      const asset = assetsById.get(assetId);
      const updatedAsset = (
        await setAssetTransformCorrection(asset.repFile.fileId, {
          rotation: (asset?.repFile.rotationCorrection + 90) % 360,
        })
      ).data;
      asset.repFile.rotationCorrection = updatedAsset.repFile.rotationCorrection;
      layoutSection(getItemForAsset(asset.assetId).pos.sectionIndex);
    },
    mirrorAsset: async (assetId: AssetId, axis: 'horizontal' | 'vertical') => {
      const asset = assetsById.get(assetId);
      const rotation = asset.repFile.rotationCorrection;
      const mirror = asset.repFile.mirrorCorrection;
      const {
        newMirror,
        newRotation,
      }: { newMirror: MirrorCorrection; newRotation?: number | undefined } = (() => {
        if (mirror === 'none') {
          return { newMirror: axis };
        } else if (mirror != axis) {
          return { newMirror: 'none', newRotation: (rotation + 180) % 360 };
        } else {
          return { newMirror: 'none' };
        }
      })();
      const updatedAsset = (
        await setAssetTransformCorrection(asset.repFile.fileId, {
          mirror: newMirror,
          rotation: newRotation,
        })
      ).data;
      asset.repFile = updatedAsset.repFile;
      layoutSection(getItemForAsset(asset.assetId).pos.sectionIndex);
    },
    get scrollbarMonths() {
      return scrollbarMonths;
    },
  };
}

function estimateHeight(
  totalNormalizedWidth: number,
  lineWidth: number,
  targetRowHeight: number,
): number {
  if (lineWidth === 0) {
    return 0;
  }
  const rows = Math.ceil((totalNormalizedWidth * targetRowHeight * 1.4) / (lineWidth * 0.8)); // consider most rows as not  filled. arbitrary
  return rows * targetRowHeight;
}

/** Filter year labels so none overlap, removing the years that are smallest in height first.
 * Simple weighted interval scheduling for very small arrays.
 * @param ys tops of labels, sorted in ascending order
 * */
function selectYearLabels(ys: number[], labelHeight: number): Set<number> {
  if (ys.length <= 2) {
    return new Set(ys.map((_, i) => i));
  }

  // weight/goal is the height that a label represents
  const weight = ys.map((y, i) => (i < ys.length - 1 ? ys[i + 1] - y : 0));
  // initial: every label by itself
  // (first label is always displayed)
  const optim = weight;
  // keep track of the path leading to the maximizer of optim
  const previous = new Array(ys.length).fill(-1);

  for (let i = 1; i < ys.length; i++) {
    // compatible just means not overlapping
    const j = ys.findLastIndex((y) => y <= ys[i] - labelHeight);
    if (j >= 0) {
      const cand = optim[j] + weight[i];
      if (cand > optim[i]) {
        optim[i] = cand;
        previous[i] = j;
      }
    }
  }

  const keep = new Set<number>();
  for (let i = ys.length - 1; i !== -1; i = previous[i]) {
    keep.add(i);
  }
  return keep;
}
