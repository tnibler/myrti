import { createContext } from 'svelte';

export type GalleryContext = {
  setAssetHidden: (assetId: string) => Promise<void>;
  setAssetSeriesSelection: (assetId: string, isSeriesSelection: boolean) => Promise<void>;
};

export const [getGalleryContext, setGalleryContext] = createContext<GalleryContext>();
