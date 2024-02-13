
export type ThumbnailBounds = {
  /** Bounds of the DOMRect that the open animation should start from */
  rect: { x: number; y: number; width: number; height: number };
  /** Rect in the image that the thumbnail shows */
  crop: { x: number; y: number; width: number; height: number } | undefined;
};

