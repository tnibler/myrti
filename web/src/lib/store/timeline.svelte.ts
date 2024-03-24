import type { Api, AssetWithSpe, TimelineSection, TimelineSegment } from "$lib/apitypes"

export type Viewport = { width: number, height: number }

export type DisplaySection = {
  section: TimelineSection,
  lastUpdateTime: number,
  height: number,
  top: number,
  segments: TimelineSegment[] | undefined,
  assetStartIndex: number
}

type LayoutConfig = {
  targetRowHeight: number,
  sectionMargin: number,
  headerHeight: number,
  segmentMargin: number,
  boxSpacing: number
}

export interface TimelineGridStore {
  initialize: (viewport: Viewport) => Promise<void>,
  loadSection: (sectionIndex: number) => void,
  /**
   * Initial section heights are estimated and only accurately computed when the assets inside are actually loaded
   * and laid out. When that happens, the actual height is updated, changing other section positions as required.
    */
  setRealSectionHeight: (sectionIndex: number, height: number) => void,
  getAssetAtIndex: (assetIndex: number) => Promise<AssetWithSpe | null>,
  preloadAssetAtIndex: (assetIndex: number) => Promise<void>
  readonly sections: DisplaySection[],
  readonly layoutConfig: LayoutConfig,
  readonly totalNumAssets: number,

  /** Maps currently selected assetIds to a number that are in order of selection (but not contiguous) */
  readonly selectedAssetIds: Record<string, number>,
  /** Assets are highlighted when something is selected and shift is pressed to preview
   * possible range selection. */
  readonly selectionPreviewIds: Record<string, boolean>,
  clearSelection: () => void;
  setAssetSelected: (assetId: string, selected: boolean) => void;
  /** @param clickedAssetIndex asset clicked to perform range selection */
  setRangeSelected: (clickedAssetIndex: number, selected: boolean) => void;
  /** Asset is hoevered while shift is pressed, selection range should be highlighted */
  rangeSelectHover: (hoveredAssetIndex: number) => void;

  hideSelectedAssets: () => Promise<void>;
}

/**
 * @param onAdjustScrollY scroll timeline container by `scrollDelta` if `scrollTop > minApplicableTop`
 * */
export function createTimeline(
  layoutConfig: LayoutConfig,
  onAdjustScrollY: (scrollDelta: number, minApplicableTop: number) => void,
  api: Api): TimelineGridStore {
  let viewport: Viewport = { width: 0, height: 0 }
  let sections: DisplaySection[] = $state([])

  const inflightSegmentRequests: Map<string, Promise<TimelineSegment[]>> = new Map();
  function requestSegments(sectionId: string): Promise<TimelineSegment[]> {
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

  async function initialize(_viewport: Viewport) {
    viewport = _viewport
    await loadSectionPlaceholders();
  }

  async function loadSectionPlaceholders() {
    const sectionsResponse = await api.getTimelineSections();
    const sectionData: TimelineSection[] = sectionsResponse.sections;

    const displaySections: DisplaySection[] = [];
    let nextSectionTop = layoutConfig.sectionMargin;
    let startIndex = 0;
    for (const section of sectionData) {
      const height = estimateHeight(section, viewport.width, layoutConfig.targetRowHeight);
      displaySections.push({
        section,
        height,
        lastUpdateTime: 0,
        segments: undefined,
        top: nextSectionTop,
        assetStartIndex: startIndex
      });
      nextSectionTop += layoutConfig.sectionMargin + height;
      startIndex += section.numAssets
    }
    sections = displaySections;
  };

  async function loadSection(sectionIndex: number) {
    if (!sections[sectionIndex].segments) {
      console.log("loading section", sectionIndex)
      const sectionId = sections[sectionIndex].section.id;
      const segments = await requestSegments(sectionId);
      sections[sectionIndex].segments = segments;
    }
  }

  function setRealSectionHeight(sectionIndex: number, height: number) {
    const oldHeight = sections[sectionIndex].height;
    const delta = height - oldHeight;
    if (delta === 0) {
      return 0;
    }
    sections[sectionIndex].height = height;
    for (let i = sectionIndex + 1; i < sections.length; i += 1) {
      sections[i].top += delta;
    }
    const minApplicableTop = sections[sectionIndex].top;
    onAdjustScrollY(delta, minApplicableTop);
  }

  function estimateHeight(section: TimelineSection, lineWidth: number, targetRowHeight: number): number {
    if (lineWidth === 0) {
      return 0;
    }
    const unwrappedWidth =
      section.avgAspectRatio * section.numAssets * targetRowHeight * (7 / 10);
    const rows = Math.ceil(unwrappedWidth / lineWidth);
    const height = rows * targetRowHeight;

    return height;
  }

  const totalNumAssets: number = $derived(sections.reduce((acc, section: DisplaySection) => acc + section.section.numAssets, 0));
  const sectionStartIndices = $derived(computeSectionStartIndices(sections));

  // TODO figure out a better way to store and access assets without iterating over sections and segments every time,
  // just store assets in a map under their index when they are loaded

  async function getAssetAtIndex(assetIndex: number): Promise<AssetWithSpe | null> {
    if (assetIndex >= totalNumAssets) {
      return null;
    }
    const sectionIndex = sections.findLastIndex((_section, idx) => {
      return sectionStartIndices[idx] <= assetIndex;
    });
    console.assert(sectionIndex >= 0);
    if (!sections[sectionIndex].segments) {
      await loadSection(sectionIndex);
    }
    const segments: TimelineSegment[] = sections[sectionIndex].segments as TimelineSegment[];

    console.assert(segments.length > 0);
    let segmentIndex = 0;
    let assetsUpToSegment = sectionStartIndices[sectionIndex];
    for (let i = 0; i < segments.length; i += 1) {
      if (assetsUpToSegment + segments[i].assets.length > assetIndex) {
        break;
      }
      assetsUpToSegment += segments[i].assets.length;
      segmentIndex += 1;
    }

    const indexInSegment = assetIndex - assetsUpToSegment;
    return segments[segmentIndex].assets[indexInSegment]
  }

  async function preloadAssetAtIndex(assetIndex: number) {
    if (assetIndex >= totalNumAssets) {
      return;
    }
    const sectionIndex = sections.findLastIndex((section, idx) => {
      return sectionStartIndices[idx] <= assetIndex;
    });
    console.assert(sectionIndex >= 0);
    if (sectionIndex >= 0 && !sections[sectionIndex].segments) {
      await loadSection(sectionIndex);
    }
  }

  let numSelected = 0;
  let nextSelectionIndex = 0;
  const selectedAssetIds: Record<string, number> = $state({});
  const selectionPreviewIds: Record<string, boolean> = $state({});

  function setAssetSelected(assetId: string, selected: boolean) {
    if (selected) {
      selectedAssetIds[assetId] = nextSelectionIndex;
      numSelected += 1;
      nextSelectionIndex += 1;
    } else {
      delete selectedAssetIds[assetId];
      numSelected -= 1;
      if (numSelected == 0) {
        nextSelectionIndex = 0;
      }
    }
  }

  function setRangeSelected(clickedIndex: number, selected: boolean) {
    console.error("TODO setRangeSelected");
  }

  function rangeSelectHover(hoveredIndex: number) {
    console.error("TODO rangeSelectHover");
  }

  async function hideSelectedAssets() {
    await api.setAssetsHidden({ what: 'hide', assetIds: Object.keys(selectedAssetIds) });
    for (let i = 0; i < sections.length; i += 1) {
      const section = sections[i];
      const segments = section.segments;
      if (!segments) {
        continue;
      }
      const segmentsToRemove: Set<number> = new Set();
      for (let j = 0; j < segments.length; j += 1) {
        const segment = segments[j];
        const remainingAssets = segment.assets.filter((asset) => !(asset.id in selectedAssetIds));
        if (remainingAssets.length === 0) {
          segmentsToRemove.add(j)
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
    }
  }

  return {
    initialize,
    loadSection,
    setRealSectionHeight,
    getAssetAtIndex,
    preloadAssetAtIndex,
    get sections() { return sections },
    get layoutConfig() { return layoutConfig },
    get totalNumAssets() { return totalNumAssets },
    setAssetSelected,
    setRangeSelected,
    rangeSelectHover,
    clearSelection: () => {
      for (const assetId in selectedAssetIds) {
        delete selectedAssetIds[assetId];
      }
    },
    get selectedAssetIds() { return selectedAssetIds },
    get selectionPreviewIds() { return selectionPreviewIds },
    hideSelectedAssets,
  };
}

function computeSectionStartIndices(sections: DisplaySection[]): number[] {
  if (sections.length == 1) {
    return [0]
  } else if (sections.length == 0) {
    return []
  }
  const idxs = [0]
  for (let i = 1; i < sections.length; i += 1) {
    idxs.push(idxs[i - 1] + sections[i - 1].section.numAssets);
  }
  return idxs
}
