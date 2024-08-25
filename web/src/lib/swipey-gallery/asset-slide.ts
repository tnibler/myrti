import type { AssetWithSpe } from '@api/myrti';
import type { SingleAssetSlide } from './gallery-types';

export function slideForAsset(asset: AssetWithSpe): SingleAssetSlide {
  if (asset.assetType === 'image') {
    return {
      assetType: 'image',
      asset,
      size: { width: asset.width, height: asset.height },
      src: '/api/assets/original/' + asset.id,
      placeholderSrc: '/api/assets/thumbnail/' + asset.id + '/large/avif',
    };
  } else {
    const videoSource = asset.hasDash
      ? { videoSource: 'dash' as const, mpdManifestUrl: '/api/dash/' + asset.id + '/stream.mpd' }
      : {
          videoSource: 'original' as const,
          mimeType: asset.mimeType,
          src: '/api/assets/original/' + asset.id,
        };
    return {
      assetType: 'video',
      asset,
      size: { width: asset.width, height: asset.height },
      placeholderSrc: '/api/assets/thumbnail/' + asset.id + '/large/avif',
      ...videoSource,
    };
  }
}
