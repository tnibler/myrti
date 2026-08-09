import type {
  AssetWithSpe,
  TimelineSection as ApiTimelineSection,
  AssetId,
  AssetSeriesId,
} from '@api/myrti';
import type { Dayjs } from '@lib/dayjs';

/** Subdivision of the timeline that is fetched from API, contains segments. */
export type TimelineSection = {
  heightEstimate: number;
  data: ApiTimelineSection;
  segments: TimelineSegment[] | null;
  /** Date of most recent asset in section */
  startDate: Dayjs;
  /** Date of oldest asset in section */
  endDate: Dayjs;
  blocks: TimelineBlock[] | null;
};

export type TimelineBlock = (
  | {
      blockType: 'default';
      titleMajor: {
        text: string;
        key: string;
      } | null;
      titlesMinor: {
        left: number;
        width: number;
        text: string;
        key: string;
      }[];
    }
  | {
      blockType: 'createGroup';
    }
) & {
  /** Height of absolute layout grid only */
  gridHeight: number;
  /** Height including grid and titles. Measured and set from DOM */
  fullHeight: number;
  gridItems: TimelineGridItem[];
};

/** A component displayed in the timeline */
export type TimelineGridItem = { key: string; top: number; height: number } & (
  | {
      type: 'asset';
      left: number;
      width: number;
      assetId: AssetId;
      timelineItem: TimelineItem;
    }
  | {
      type: 'photoStack';
      left: number;
      width: number;
      seriesId: AssetSeriesId;
      coverIndex: number;
      numAssets: number;
      timelineItem: TimelineItem;
    }
);

/** Logical group of assets (eg belonging to the same date, in the process of creating a group)
 * that are laid out together */
export type TimelineSegment = {
  type: string;
  items: TimelineItem[];
  sortDate: string;
  start: Dayjs;
  end: Dayjs;
} & (
  | {
      type: 'dateRange';
    }
  | {
      type: 'group';
      title: string;
      groupId: string;
      clickArea: AddToGroupClickArea | null;
    }
  | { type: 'creatingGroup' }
);

/** Backlink from TimelineItem to its segment/section */
export type PositionInTimeline = {
  sectionIndex: number;
  segmentIndex: number;
  itemIndex: number;
};

/** One asset or a photo series shown in the timeline, basically whatever is displayed as a small image */
export type TimelineItem = {
  pos: PositionInTimeline;
  key: string;
  sortDate: string;
} & (
  | {
      /** A single asset */
      itemType: 'asset';
      assetId: AssetId;
    }
  | {
      /** Complete or split up stack. If a stack has multiple images marked as good, the stack is split up at each marked image. */
      itemType: 'photoStack';
      seriesId: AssetSeriesId;
      /** `series.assets[coverIndex]` is the cover image shown in the timelinew
       * for this (portion of a) stack */
      coverIndex: number;
      /** This piece of the series "contains" series.assets[splitStart..splitEnd] */
      splitStart: number;
      splitEnd: number;
    }
);

export type AssetSeries = {
  seriesId: string;
  assets: AssetWithSpe[];
  selectionIndices: number[];
};

/** When creating a new group, existing groups become clickable to add the current selection to them */
export type AddToGroupClickArea = {
  groupId: string;
  top: number;
  left: number;
  width: number;
  height: number;
};

export type OpenedSlide =
  | (TimelineItem & { itemType: 'asset' })
  | (TimelineItem & { itemType: 'photoStack'; seriesIndex: number });
