import type { AssetWithSpe, TimelineSection as ApiTimelineSection } from '@api/myrti';
import type { Dayjs } from '@lib/dayjs';

export type ItemRange = { startIdx: number; endIdx: number };

/** Subdivision of the timeline that is fetched from API, contains segments. */
export type TimelineSection = {
  top: number;
  height: number;
  data: ApiTimelineSection;
  segments: TimelineSegment[] | null;
  items: ItemRange | null;
  /** Date of most recent asset in section */
  startDate: Dayjs;
  /** Date of oldest asset in section */
  endDate: Dayjs;
};

/** Logical group of assets (eg belonging to the same date, in the process of creating a group)
 * that are laid out together */
export type TimelineSegment = {
  type: string;
  items: TimelineItem[];
  sortDate: string;
  start: Dayjs;
  end: Dayjs;
  itemRange: ItemRange | null;
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

/** One asset or a photo series shown in the timeline, basically whatever is displayed as a small image */
export type TimelineItem =
  | ({
      /** A single asset */
      itemType: 'asset';
      /** Index within all assets in timeline (counting each asset in a stack as well) */
      assetIndex: number;
    } & AssetWithSpe)
  | {
      /** Complete or split up stack. If a stack has multiple images marked as good, the stack is split up at each marked image. */
      itemType: 'photoStack';
      series: AssetSeries;
      /** `series.assets[coverIndex]` is the cover image shown in the timelinew
       * for this (portion of a) stack */
      coverIndex: number;
      /** Index of series.assets[0] within all assets in timeline (counting each asset in a stack as well) */
      firstAssetIndex: number;
    };

export type AssetSeries = {
  seriesId: string;
  assets: AssetWithSpe[];
  selectionIndices: number[];
};

/** When creating a new group, existing groups become clickable to add the current selection to them */
export type AddToGroupClickArea = {
  top: number;
  height: number;
  groupId: string;
};
