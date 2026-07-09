import type { AssetWithSpe } from '@api/myrti';
import type { SingleAssetSlide } from './gallery-types';

export function slideForAsset(asset: AssetWithSpe): SingleAssetSlide {
  if (asset.assetType === 'image') {
    const supported = ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'avif'];
    const src = (() => {
      const supportedRepr = asset.representations.find(
        (repr) => supported.indexOf(repr.format) >= 0,
      );
      if (
        supported.map((format) => 'image/' + format).indexOf(asset.mimeType) >= 0 ||
        !supportedRepr
      ) {
        return '/api/assets/original/' + asset.id;
      }
      return `/api/assets/repr/${asset.id}/${supportedRepr.id}`;
    })();
    return {
      assetType: 'image',
      asset,
      size: { width: asset.width, height: asset.height },
      src,
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
