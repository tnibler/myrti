import type { AssetWithSpe } from "$lib/apitypes";
import type { Size } from "./util_types";

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
} & ({ videoSource: 'dash', mpdManifestUrl: string } | { videoSource: 'original', mimeType: string })

export type SlideData = ImageSlideData | VideoSlideData;

export function slideForAsset(asset: AssetWithSpe): SlideData {
  if (asset.type === 'image') {
    return {
      type: 'image',
      size: {
        width: asset.width,
        height: asset.height
      },
      src: '/api/asset/original/' + asset.id,
      placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif'
    };
  } else {
    const videoSource = asset.hasDash
      ? {
        videoSource: 'dash' as const,
        mpdManifestUrl: `/api/dash/${asset.id}/stream.mpd`
      }
      : {
        videoSource: 'original' as const,
        mimeType: asset.mimeType
      };
    return {
      type: 'video',
      src: '/api/asset/original/' + asset.id,
      placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
      size: {
        width: asset.width,
        height: asset.height
      },
      ...videoSource
    };
  }
}
