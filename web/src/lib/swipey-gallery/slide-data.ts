import type { AssetWithSpe, ImageRepresentation } from '@lib/apitypes';
import type { Size } from './util_types';

export type ImageSlideData = {
  type: 'image';
  src: string;
  placeholderSrc: string;
  size: Size;
};

export type VideoSlideData = {
  type: 'video';
  src: string;
  placeholderSrc: string;
  size: Size;
} & (
  | { videoSource: 'dash'; mpdManifestUrl: string }
  | { videoSource: 'original'; mimeType: string }
);

export type SlideData = { asset: AssetWithSpe } & (ImageSlideData | VideoSlideData);

export function slideForAsset(asset: AssetWithSpe): SlideData {
  if (asset.type === 'image') {
    const acceptedFormats = ['image/jpeg', 'image/avif', 'image/webp', 'image/png', 'image/gif'];
    let imageSrc = '/api/asset/original/' + asset.id;
    if (!acceptedFormats.some((f) => asset.mimeType === f)) {
      const reprs = asset.representations as ImageRepresentation[];
      if (reprs.length > 0) {
        imageSrc = '/api/asset/repr/' + asset.id + '/' + reprs[0].id;
      }
    }
    return {
      asset,
      type: 'image',
      size: {
        width: asset.width,
        height: asset.height,
      },
      src: imageSrc,
      placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
    };
  } else {
    const videoSource = asset.hasDash
      ? {
          videoSource: 'dash' as const,
          mpdManifestUrl: `/api/dash/${asset.id}/stream.mpd`,
        }
      : {
          videoSource: 'original' as const,
          mimeType: asset.mimeType,
        };
    return {
      asset,
      type: 'video',
      src: '/api/asset/original/' + asset.id,
      placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
      size: {
        width: asset.width,
        height: asset.height,
      },
      ...videoSource,
    };
  }
}
