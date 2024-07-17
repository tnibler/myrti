import type {
  Api,
  AssetId,
  AssetWithSpe,
  TimelineSection as ApiTimelineSection,
  TimelineSegment as ApiTimelineSegment,
} from '@lib/apitypes';
import createJustifiedLayout from 'justified-layout';
import { klona } from 'klona';
import { SvelteMap } from 'svelte/reactivity';

export type TimelineGridItem = { key: string; top: number; height: number } & (
  | {
      type: 'asset';
      left: number;
      width: number;
      /** Index (only counting 'asset' Items) in timeline  */
      assetIndex: number;
      asset: AssetWithSpe;
    }
  | {
      type: 'assetPlaceholder';
      left: number;
      width: number;
    }
  | {
      type: 'segmentTitle';
    }
  | {
      type: 'createGroupTitleInput';
    }
);

export type Viewport = { width: number; height: number };

export type ItemRange = { startIdx: number; endIdx: number };

// maybe make data field optional and only set when segment corresponds 1:1 to segment from api?
type TimelineSegment = {
  type: string;
  assets: AssetWithSpe[];
  sortDate: string;
  /** Item indices with TimelineSection.items.startIdx as base */
  itemRange: ItemRange | null;
} & (
  | {
      type: 'dateRange';
      data: ApiTimelineSegment & { type: 'dateRange' };
    }
  | {
      type: 'userGroup';
      data: ApiTimelineSegment & { type: 'userGroup' };
      clickArea: AddToGroupClickArea | null;
    }
  | {
      type: 'creatingGroup';
    }
);

type AddToGroupClickArea = {
  top: number;
  height: number;
  groupId: string;
};

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
  /** Maps currently selected assetIds to a number that are in order of selection (but not contiguous) */
  readonly selectedAssets: Map<AssetId, number>;
  readonly addToGroupClickAreas: AddToGroupClickArea[];
  // /** Assets are highlighted when something is selected and shift is pressed to preview
  //  * possible range selection. */
  // readonly selectionPreviewIds: Map<AssetId, boolean>;

  initialize: (viewport: Viewport) => Promise<void>;
  resize: (viewport: Viewport, scrollTop: number) => void;
  set setAnimationsEnabled(v: ((enabled: boolean) => Promise<void>) | null);
  onScrollChange: (top: number) => void;
  moveViewToAsset: (assetIndex: number) => Promise<TimelineGridItem | null>;
  setActualItemHeight: (itemIndex: number, newHeight: number) => void;
  getOrLoadAssetAtIndex: (index: number) => Promise<AssetWithSpe | null>;
  clearSelection: () => void;
  setAssetSelected: (assetId: string, selected: boolean) => void;
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

export type TimelineSection = {
  top: number;
  height: number;
  data: ApiTimelineSection;
  segments: TimelineSegment[] | null;
  items: ItemRange | null;
};

type TimelineState =
  | { state: 'justLooking' }
  | {
      state: 'creatingTimelineGroup';
      assetsInGroup: AssetId[];
      groupSortDate: string;
      previousItems: TimelineGridItem[];
      previousSections: TimelineSection[];
    };

export function createTimeline(
  opts: TimelineOptions,
  adjustScrollTop: (scrollDelta: number, ifScrollTopGt: number) => void,
  api: Api,
): ITimelineGrid {
  let isInitialized = false;
  let viewport: Viewport = { width: 0, height: 0 };
  let state: TimelineState = $state({ state: 'justLooking' });
  let items: TimelineGridItem[] = $state([]);
  let timelineHeight: number = $state(0);
  let sections: TimelineSection[] = $state([]);
  let addToGroupClickAreas: AddToGroupClickArea[] = $derived(
    state.state === 'creatingTimelineGroup'
      ? sections
          .filter((s) => s.segments !== null && s.items != null)
          .map((s) =>
            s
              .segments!.filter((seg) => seg.type === 'userGroup')
              .map((seg) => seg.clickArea)
              .filter((area) => area !== null),
          )
          .flat()
      : [],
  );
  let visibleItems: ItemRange = $state({ startIdx: 0, endIdx: 0 });
  let setAnimationsEnabled: ((enabled: boolean) => Promise<void>) | null = null;
  const selectedAssets: Map<AssetId, number> = $state(new SvelteMap());
  const totalNumAssets: number = $derived(
    sections.reduce((acc: number, section) => {
      return acc + section.data.numAssets;
    }, 0),
  );
  const sectionStartIndices = $derived.by(() => computeSectionStartIndices(sections));
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
        const r = await api.getTimelineSegments({ params: { id: sectionId } });
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
    const sectionsResponse = await api.getTimelineSections();
    const sectionData: ApiTimelineSection[] = sectionsResponse.sections;

    const _sections: TimelineSection[] = [];
    let nextSectionTop = 0;
    let totalHeight = 0;
    for (const section of sectionData) {
      const height = estimateHeight(section, viewport.width, opts.targetRowHeight);
      totalHeight += height;
      _sections.push({
        data: section,
        height,
        top: nextSectionTop,
        segments: null,
        items: null,
      });
      nextSectionTop += height;
    }
    sections = _sections;
    timelineHeight = totalHeight;
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
    const {
      items: sectionItems,
      sectionHeight,
      segmentItemRanges,
    } = populateSection(segments, section.top, baseAssetIndex, viewport.width);
    const oldSectionHeight = sections[sectionIndex].height;
    section.height = sectionHeight;
    for (let i = 0; i < segments.length; i += 1) {
      const segment = segments[i];
      // item indices relative to this section's startIdx
      segment.itemRange = segmentItemRanges[i];
      // set group's click area
      if (segment.type === 'userGroup') {
        let currentTop = Infinity;
        let currentBottom = -Infinity;
        for (let i = segment.itemRange.startIdx; i < segment.itemRange.endIdx; i += 1) {
          const item = sectionItems[i];
          currentTop = Math.min(item.top, currentTop);
          currentBottom = Math.max(item.top + item.height, currentBottom);
        }
        segment.clickArea = {
          top: currentTop,
          height: currentBottom - currentTop,
          groupId: segment.data.id,
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
      const mustAdjustScrollIfScrollTopGt = sections[sectionIndex].top;
      adjustScrollTop(heightDelta, mustAdjustScrollIfScrollTopGt);
    }
  }

  async function loadSection(sectionIndex: number) {
    const section = sections[sectionIndex];
    if (section.segments != null) {
      return;
    }
    const sectionId = section.data.id;
    const segments = await requestSegments(sectionId);
    sections[sectionIndex].data.segments = segments;
    sections[sectionIndex].segments = segments.map((segment) => {
      if (segment.type === 'dateRange') {
        return {
          type: segment.type,
          data: segment,
          assets: segment.assets,
          sortDate: segment.sortDate,
          itemRange: null,
        };
      }
      return {
        type: segment.type,
        data: segment,
        assets: segment.assets,
        sortDate: segment.sortDate,
        itemRange: null,
        clickArea: null,
      };
    });
  }

  /** Increasing number to track order in which assets are selected. Used for values of selectedAssets */
  let nextSelectionIndex = 0;
  function setAssetSelected(assetId: AssetId, selected: boolean) {
    if (selected) {
      selectedAssets.set(assetId, nextSelectionIndex);
      nextSelectionIndex += 1;
    } else {
      selectedAssets.delete(assetId);
    }
  }

  function clearSelection() {
    nextSelectionIndex = 0;
    selectedAssets.clear();
  }

  async function hideSelectedAssets() {
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    await api.setAssetsHidden({ what: 'hide', assetIds: Array.from(selectedAssets.keys()) });
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
        const segment = segments[segmentIdx];
        const remainingAssets = segment.assets.filter((asset) => !selectedAssets.has(asset.id));
        if (
          remainingAssets.length != segment.assets.length &&
          ((affectedSectionIdxs.length > 0 && affectedSectionIdxs.at(-1) != sectionIdx) ||
            affectedSectionIdxs.length == 0)
        ) {
          affectedSectionIdxs.push(sectionIdx);
        }
        newNumAssets += remainingAssets.length;
        if (remainingAssets.length === 0) {
          segmentsToRemove.add(segmentIdx);
        } else {
          segment.assets = remainingAssets;
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
    visibleItems.endIdx -= selectedAssets.size; // not 100% sure on this
    selectedAssets.clear();
    // reassign Items' asset index
    items.filter((item) => item.type === 'asset').forEach((item, idx) => (item.assetIndex = idx));
    for (const sectionIdx of affectedSectionIdxs) {
      layoutSection(sectionIdx, 'noAdjustScroll');
    }
    setTimeout(() => {
      if (setAnimationsEnabled) {
        setAnimationsEnabled(false);
      }
    }, 500);
  }

  async function getOrLoadAssetAtIndex(index: number): Promise<AssetWithSpe | null> {
    if (index >= totalNumAssets) {
      console.error(`ask for getAssetAtIndex(${index}) but only ${totalNumAssets} in total`);
      return null;
    }
    const sectionIndex = sections.findLastIndex((_section, idx) => {
      return sectionStartIndices[idx] <= index;
    });
    console.assert(sectionIndex >= 0);
    if (!sections[sectionIndex].segments) {
      await loadSection(sectionIndex);
    }
    const segments = sections[sectionIndex].segments;
    if (segments === null) {
      console.error('getAssetAtIndex: segments still null after loading section');
      return null;
    }

    console.assert(segments.length > 0);
    let segmentIndex = 0;
    let assetsUpToSegment = sectionStartIndices[sectionIndex];
    for (let i = 0; i < segments.length; i += 1) {
      if (assetsUpToSegment + segments[i].assets.length > index) {
        break;
      }
      assetsUpToSegment += segments[i].assets.length;
      segmentIndex += 1;
    }

    const indexInSegment = index - assetsUpToSegment;
    return segments[segmentIndex].assets[indexInSegment];
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
    const heightDelta = newHeight - item.height;
    sections[sectionIndex].height += heightDelta;
    items[itemIndex].height += heightDelta;
    // items[i].top <= items[i+1].top, so shift starting from itemIndex onwards
    for (let i = itemIndex + 1; i < sections[sectionIndex].items.endIdx; i += 1) {
      items[i].top += heightDelta;
    }
    adjustScrollTop(heightDelta, item.top);
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
        if (segment.type === 'dateRange' || segment.type === 'userGroup') {
          const segmentTitleKey =
            segment.type === 'dateRange'
              ? `${segment.data.start}-${segment.data.end}`
              : `group-${segment.data.id}`;
          return {
            type: 'segmentTitle',
            top: nextSegmentTop,
            height: headerHeight,
            key: segmentTitleKey,
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

  async function moveViewToAsset(assetIndex: number): Promise<TimelineGridItem | null> {
    let sectionIndex = -1;
    // find section containing asset
    let cumulAssets = 0;
    for (let i = 0; i < sections.length; i += 1) {
      cumulAssets += sections[i].data.numAssets;
      if (assetIndex < cumulAssets) {
        sectionIndex = i;
        break;
      }
    }
    if (sectionIndex < 0) {
      console.error('scrollToAssetIndex: did not find section containing asset at index');
      return null;
    }
    const section = sections[sectionIndex];
    // fetch section data from api if necessary
    await loadSection(sectionIndex);
    // compute layouts for segments in section, populating items array
    layoutSection(sectionIndex, 'noAdjustScroll');
    console.assert(section.items !== null, 'loaded section but items === null');
    if (section.items === null) {
      return null;
    }
    // find item in items array
    let itemIndex = -1;
    for (let i = section.items.startIdx; i < section.items.endIdx; i += 1) {
      const item = items[i];
      if (item.type === 'asset' && item.assetIndex === assetIndex) {
        itemIndex = i;
        break;
      }
    }
    console.assert(itemIndex >= 0, 'loaded and laid out section but did not find correct item');
    if (itemIndex < 0) {
      return null;
    }
    return items[itemIndex];
  }

  let groupNumber = 0;
  async function createGroupClicked() {
    const previousSections = klona(sections);
    const previousItems = klona(items);
    const selected = new Set(selectedAssets.keys());
    if (selected.size === 0) {
      return;
    }
    clearSelection();
    const assetsInGroup: AssetWithSpe[] = [];
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
        // arrays of contiguous assets, which may be separated by assets in group
        const remainingAssets: AssetWithSpe[][] = [];
        let currentlyInGroup = false;
        for (const asset of segment.assets) {
          if (selected.has(asset.id)) {
            currentlyInGroup = true;
            thisSectionAffected = true;
            assetsInGroup.push(asset);
          } else {
            if (currentlyInGroup || remainingAssets.length === 0) {
              currentlyInGroup = false;
              remainingAssets.push([asset]);
            } else {
              remainingAssets.at(-1)!.push(asset);
            }
          }
        }
        if (remainingAssets.length === 1 && remainingAssets[0].length > 0) {
          const newSegment: TimelineSegment = {
            type: 'dateRange',
            assets: remainingAssets[0],
            sortDate: remainingAssets[0][0].takenDate,
            itemRange: null,
            data: {
              type: 'dateRange',
              id: segment.data.id,
              assets: remainingAssets[0],
              sortDate: remainingAssets[0][0].takenDate,
              end: remainingAssets[0][0].takenDate,
              start: remainingAssets[0].at(-1)!.takenDate,
            },
          };
          newSegments.push(newSegment);
        } else {
          for (const [i, assets] of remainingAssets.entries()) {
            console.assert(assets.length > 0);
            const newSegment: TimelineSegment = {
              type: 'dateRange',
              assets,
              sortDate: assets[0].takenDate,
              itemRange: null,
              data: {
                type: 'dateRange',
                id: segment.data.id + '_' + i,
                assets: assets,
                sortDate: assets[0].takenDate,
                start: assets.at(-1)!.takenDate,
                end: assets[0].takenDate,
              },
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
    const groupSortDate = assetsInGroup.at(-1)?.takenDate;
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
      (s) => s.assets.at(0)!.takenDate < groupSortDate,
    );
    const newSegment: TimelineSegment & { type: 'creatingGroup' } = $state({
      type: 'creatingGroup',
      assets: assetsInGroup,
      sortDate: groupSortDate,
      itemRange: null,
    });
    groupNumber += 1;
    section.segments!.splice(insertBeforeSegmentIndex, 0, newSegment);
    if (setAnimationsEnabled) {
      await setAnimationsEnabled(true);
    }
    for (const i of affectedSections) {
      layoutSection(i, 'noAdjustScroll');
    }
    setTimeout(() => {
      if (setAnimationsEnabled) {
        setAnimationsEnabled(true);
      }
    }, 500);
    state = {
      state: 'creatingTimelineGroup',
      assetsInGroup: assetsInGroup.map((a) => a.id),
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
    setTimeout(() => {
      if (setAnimationsEnabled) {
        setAnimationsEnabled(true);
      }
    }, 500);
  }

  async function confirmCreateGroup(title: string): Promise<void> {
    if (state.state !== 'creatingTimelineGroup') {
      return;
    }
    const response = await api.createTimelineGroup({ name: title, assets: state.assetsInGroup });
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
      type: 'userGroup',
      assets: oldSegment.assets,
      sortDate: response.displayDate,
      itemRange: null,
      data: {
        id: response.timelineGroupId,
        sortDate: response.displayDate,
        assets: oldSegment.assets,
        type: 'userGroup',
        name: title,
      },
    };
    // TODO: track scroll (also in createGroupClicked), enable animations
    layoutSection(sectionIndex, 'adjustScroll');
    state = { state: 'justLooking' };
  }

  async function addSelectedToExistingGroup(groupId: string): Promise<void> {
    // make it so we don't have to loop over everything to know what sections etc
    // are affected and what to relayout.
    // same thing for createGroupClicked
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
    get selectedAssets() {
      return selectedAssets;
    },
    get options() {
      return opts;
    },
    set setAnimationsEnabled(v: ((enabled: boolean) => Promise<void>) | null) {
      setAnimationsEnabled = v;
    },
    initialize,
    resize,
    onScrollChange,
    moveViewToAsset,
    setActualItemHeight,
    getOrLoadAssetAtIndex,
    setAssetSelected,
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

function computeSectionStartIndices(sections: TimelineSection[]): number[] {
  if (sections.length == 1) {
    return [0];
  } else if (sections.length == 0) {
    return [];
  }
  const idxs = [0];
  for (let i = 1; i < sections.length; i += 1) {
    idxs.push(idxs[i - 1] + sections[i - 1].data.numAssets);
  }
  return idxs;
}
