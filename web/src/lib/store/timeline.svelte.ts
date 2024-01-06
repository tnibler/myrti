import type { Api, TimelineSection, TimelineSegment } from "$lib/apitypes"

export type Viewport = { width: number, height: number }

export type DisplaySection = {
  section: TimelineSection,
  lastUpdateTime: number,
  height: number,
  top: number,
  segments: TimelineSegment[] | undefined
}

type LayoutConfig = { targetRowHeight: number, sectionMargin: number }

export interface TimelineGrid {
  initialize: (viewport: Viewport) => Promise<void>,
  loadSection: (sectionIndex: number) => void,
  setRealSectionHeight: (sectionIndex: number, height: number) => void,
  readonly sections: DisplaySection[]
  readonly layoutConfig: LayoutConfig
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
    for (const section of sectionData) {
      const height = estimateHeight(section, viewport.width, layoutConfig.targetRowHeight);
      displaySections.push({
        section,
        height,
        lastUpdateTime: 0,
        segments: undefined,
        top: nextSectionTop
      });
      nextSectionTop += layoutConfig.sectionMargin + height;
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

  return {
    initialize,
    loadSection,
    setRealSectionHeight,
    get sections() { return sections },
    get layoutConfig() { return layoutConfig }
  }
}
