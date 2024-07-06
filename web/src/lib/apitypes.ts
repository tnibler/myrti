import type { z } from 'zod';
import type { api, schemas } from './apiclient';

export type Api = typeof api;

export type Album = z.infer<typeof schemas.Album>;
export type AlbumItem = z.infer<typeof schemas.AlbumItem>;
export type AssetWithSpe = z.infer<typeof schemas.AssetWithSpe>;
export type Asset = z.infer<typeof schemas.Asset>;
export type AssetId = z.infer<typeof schemas.AssetId>;
export type TimelineGroup = z.infer<typeof schemas.TimelineGroup>;
export type TimelineSection = z.infer<typeof schemas.TimelineSection>;
export type TimelineSegment = z.infer<typeof schemas.TimelineSegment>;
export type ImageRepresentation = z.infer<typeof schemas.ImageRepresentation>;
