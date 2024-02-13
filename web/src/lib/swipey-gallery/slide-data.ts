import type { Size } from "./util_types";

export type ImageSlideData = {
  type: 'image';
  src: string;
  placeholderSrc: string;
  size: Size;
};

export type VideoSlideData = {
  type: 'image';
  src: string;
  placeholderSrc: string;
  size: Size;
  mimeType: string;
}

export type SlideData = ImageSlideData | VideoSlideData;
