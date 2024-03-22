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
  mimeType: string;
} & ({ videoSource: 'dash', mpdManifestUrl: string } | { videoSource: 'original' })

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
    const videoSource:
      | { videoSource: 'dash'; mpdManifestUrl: string }
      | { videoSource: 'original' } = asset.hasDash
        ? {
          videoSource: 'dash',
          mpdManifestUrl: `/api/dash/${asset.id}/stream.mpd`
        }
        : { videoSource: 'original' };
    return {
      type: 'video',
      src: '/api/asset/original/' + asset.id,
      placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
      size: {
        width: asset.width,
        height: asset.height
      },
      mimeType: asset.mimeType, // FIXME this is not actually part of Asset reponses
      ...videoSource
    };
  }
}
