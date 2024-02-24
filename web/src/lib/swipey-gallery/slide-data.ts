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
