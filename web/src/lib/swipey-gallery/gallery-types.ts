import type { AssetId, AssetSeriesId, AssetWithSpe } from '@api/myrti';
import type { AssetSeries } from '@lib/timeline-grid/timeline-types';
import type { Size } from './util_types';
import type { Readable } from 'svelte/store';

export type GallerySlide<Pos> = {
  pos: Pos;
} & GallerySlideData;

export type GallerySlideData =
  | ({
      slideType: 'singleAsset';
    } & SingleAssetSlide)
  | {
      slideType: 'assetSeries';
      series: AssetSeries;
      coverIndex: number;
      coverSlide: SingleAssetSlide;
    };

export type ImageSlideData = {
  assetType: 'image';
  asset: AssetWithSpe & {
    assetType: 'image';
  };
  src: string;
  placeholderSrc: string;
  size: Size;
};

export type VideoSlideData = {
  assetType: 'video';
  asset: AssetWithSpe & {
    assetType: 'video';
  };
  placeholderSrc: string;
  size: Size;
} & (
  | {
      videoSource: 'original';
      mimeType: string;
      src: string;
    }
  | {
      videoSource: 'dash';
      mpdManifestUrl: string;
    }
);

export type SingleAssetSlide = ImageSlideData | VideoSlideData;

export type SlideRef =
  | {
      slideType: 'singleAsset';
      assetId: AssetId;
    }
  | {
      slideType: 'assetSeries';
      assetSeriesId: AssetSeriesId;
      coverIndex: number;
    };

export interface GalleryDataSource {
  asset(id: AssetId): Readable<AssetWithSpe | null>;
  assetSeries(id: AssetSeriesId): Readable<AssetSeries | null>;

  nextSlide(s: SlideRef, dir: 'left' | 'right'): Readable<SlideRef | null>;
}
