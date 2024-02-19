import type { Api, Asset, TimelineSection, TimelineSegment } from "$lib/apitypes"

export type Viewport = { width: number, height: number }

export type DisplaySection = {
  section: TimelineSection,
  lastUpdateTime: number,
  height: number,
  top: number,
  segments: TimelineSegment[] | undefined,
  assetStartIndex: number
}

type LayoutConfig = { targetRowHeight: number, sectionMargin: number }

export interface TimelineGrid {
  initialize: (viewport: Viewport) => Promise<void>,
  loadSection: (sectionIndex: number) => void,
  setRealSectionHeight: (sectionIndex: number, height: number) => void,
  getAssetAtIndex: (assetIndex: number) => Promise<Asset | null>,
  loadAssetAtIndex: (assetIndex: number) => Promise<void>
  readonly sections: DisplaySection[],
  readonly layoutConfig: LayoutConfig,
  readonly totalNumAssets: number,
}

export function createTimeline(layoutConfig: LayoutConfig, api: Api): TimelineGrid {
  let viewport: Viewport = { width: 0, height: 0 }
  let sections: DisplaySection[] = $state([])

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
      const segmentResponse = await api.getTimelineSegments({ queries: { sectionId } });
      sections[sectionIndex].segments = segmentResponse.segments;
    }
  }

  /**
   * Initial section heights are estimated and only accurately computed when the assets inside are actually loaded
   * and laid out. When that happens, the actual height is updated, changing other section positions as required.
   *
   * @returns Y scroll distance required to compensate for the change in section heights if `window.scrollY > sections[sectionIndex].top`
    */
  function setRealSectionHeight(sectionIndex: number, height: number): number {
    const oldHeight = sections[sectionIndex].height;
    const delta = height - oldHeight;
    if (delta === 0) {
      return 0;
    }
    sections[sectionIndex].height = height;
    for (let i = sectionIndex + 1; i < sections.length; i += 1) {
      sections[i].top += delta;
    }
    return delta;
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

  async function getAssetAtIndex(assetIndex: number): Promise<Asset | null> {
    console.log("get asset index", assetIndex)
    if (assetIndex >= totalNumAssets) {
      return null;
    }
    const sectionIndex = sections.findLastIndex((_section, idx) => {
      return sectionStartIndices[idx] <= assetIndex;
    });
    console.assert(sectionIndex >= 0);
    if (!sections[sectionIndex].segments) {
      console.log("not loaded", sectionIndex)
      return undefined;
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

  async function loadAssetAtIndex(assetIndex: number) {
    if (assetIndex >= totalNumAssets) {
      return undefined
    }
    const sectionIndex = sections.findLastIndex((section, idx) => {
      return sectionStartIndices[idx] <= assetIndex;
    });
    console.assert(sectionIndex >= 0);
    if (!sections[sectionIndex].segments) {
      await loadSection(sectionIndex);
    }
  }

  return {
    initialize,
    loadSection,
    setRealSectionHeight,
    getAssetAtIndex,
    loadAssetAtIndex,
    get sections() { return sections },
    get layoutConfig() { return layoutConfig },
    get totalNumAssets() { return totalNumAssets },
  }
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
