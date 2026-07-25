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
        supported.map((format) => 'image/' + format).indexOf(asset.repFile.mimeType) >= 0 ||
        !supportedRepr
      ) {
        return '/api/files/original/' + asset.repFile.fileId;
      }
      return `/api/files/repr/${asset.repFile.fileId}/${supportedRepr.id}`;
    })();
    return {
      assetType: 'image',
      asset,
      size: { width: asset.repFile.width, height: asset.repFile.height },
      src,
      placeholderSrc: '/api/files/thumbnail/' + asset.repFile.fileId + '/large/avif',
    };
  } else {
    const videoSource =
      asset.hasDash || true
        ? {
            videoSource: 'dash' as const,
            mpdManifestUrl: '/api/dash/' + asset.repFile.fileId + '/stream.mpd',
          }
        : {
            videoSource: 'original' as const,
            mimeType: asset.repFile.mimeType,
            src: '/api/files/original/' + asset.repFile.fileId,
          };
    return {
      assetType: 'video',
      asset,
      size: { width: asset.repFile.width, height: asset.repFile.height },
      placeholderSrc: '/api/files/thumbnail/' + asset.repFile.fileId + '/large/avif',
      ...videoSource,
    };
  }
}
