import { makeApi, Zodios, type ZodiosOptions } from '@zodios/core';
import { z } from 'zod';

const AlbumId = z.string();
const Album = z
	.object({
		changedAt: z.string().datetime({ offset: true }),
		createdAt: z.string().datetime({ offset: true }),
		description: z.string().nullish(),
		id: AlbumId,
		name: z.string().nullish(),
		numAssets: z.number().int()
	})
	.passthrough();
const AssetId = z.string();
const CreateAlbumRequest = z
	.object({ assets: z.array(AssetId), description: z.string().nullish(), name: z.string() })
	.passthrough();
const CreateAlbumResponse = z.object({ albumId: z.number().int() }).passthrough();
const AssetRootDirId = z.string();
const AssetMetadataType = z.union([
	z
		.object({ Video: z.object({ duration: z.number().int().nullable() }).partial().passthrough() })
		.passthrough(),
	z
		.object({ Image: z.object({ format: z.string().nullable() }).partial().passthrough() })
		.passthrough()
]);
const AssetMetadata = AssetMetadataType.and(
	z
		.object({
			height: z.number().int().nullable(),
			taken_date: z.string().datetime({ offset: true }).nullable(),
			width: z.number().int().nullable()
		})
		.partial()
		.passthrough()
);
const AssetType = z.enum(['image', 'video']);
const Asset = z
	.object({
		addedAt: z.string().datetime({ offset: true }),
		assetRootId: AssetRootDirId,
		height: z.number().int(),
		id: AssetId,
		metadata: AssetMetadata.nullish(),
		mimeType: z.string(),
		pathInRoot: z.string(),
		takenDate: z.string().datetime({ offset: true }),
		type: AssetType,
		width: z.number().int()
	})
	.passthrough();
const AlbumDetailsResponse = z
	.object({ assets: z.array(Asset), description: z.string().nullish(), name: z.string().nullish() })
	.passthrough();
const AppendAssetsRequest = z.object({ assetIds: z.array(AssetId) }).passthrough();
const AppendAssetsResponse = z.object({ success: z.boolean() }).passthrough();
const TimelineGroupType = z.discriminatedUnion('type', [
	z.object({ date: z.string(), type: z.literal('day') }).passthrough(),
	z
		.object({
			groupEndDate: z.string().datetime({ offset: true }),
			groupId: z.string(),
			groupStartDate: z.string().datetime({ offset: true }),
			groupTitle: z.string(),
			type: z.literal('group')
		})
		.passthrough()
]);
const ImageRepresentation = z
	.object({
		format: z.string(),
		height: z.number().int(),
		id: z.string(),
		size: z.number().int(),
		width: z.number().int()
	})
	.passthrough();
const Image = z.object({ representations: z.array(ImageRepresentation) }).passthrough();
const Video = z.object({ hasDash: z.boolean() }).passthrough();
const AssetSpe = z.union([Image, Video]);
const AssetWithSpe = Asset.and(AssetSpe).and(z.object({}).partial().passthrough());
const TimelineGroup = TimelineGroupType.and(
	z.object({ assets: z.array(AssetWithSpe) }).passthrough()
);
const TimelineChunk = z
	.object({
		changedSinceLastFetch: z.boolean(),
		date: z.string().datetime({ offset: true }),
		groups: z.array(TimelineGroup)
	})
	.passthrough();
const TimelineSection = z
	.object({ avgAspectRatio: z.number(), id: z.string(), numAssets: z.number().int() })
	.passthrough();
const TimelineSectionsResponse = z.object({ sections: z.array(TimelineSection) }).passthrough();
const TimelineGroupId = z.string();
const SegmentType = z.discriminatedUnion('type', [
	z
		.object({
			end: z.string().datetime({ offset: true }),
			start: z.string().datetime({ offset: true }),
			type: z.literal('dateRange')
		})
		.passthrough(),
	z
		.object({ id: TimelineGroupId, name: z.string().nullish(), type: z.literal('userGroup') })
		.passthrough()
]);
const TimelineSegment = z
	.object({ assets: z.array(AssetWithSpe), segment: SegmentType })
	.passthrough();
const TimelineSegmentsResponse = z.object({ segments: z.array(TimelineSegment) }).passthrough();
const ThumbnailFormat = z.enum(['avif', 'webp']);
const ThumbnailSize = z.enum(['small', 'large']);

export const schemas = {
	AlbumId,
	Album,
	AssetId,
	CreateAlbumRequest,
	CreateAlbumResponse,
	AssetRootDirId,
	AssetMetadataType,
	AssetMetadata,
	AssetType,
	Asset,
	AlbumDetailsResponse,
	AppendAssetsRequest,
	AppendAssetsResponse,
	TimelineGroupType,
	ImageRepresentation,
	Image,
	Video,
	AssetSpe,
	AssetWithSpe,
	TimelineGroup,
	TimelineChunk,
	TimelineSection,
	TimelineSectionsResponse,
	TimelineGroupId,
	SegmentType,
	TimelineSegment,
	TimelineSegmentsResponse,
	ThumbnailFormat,
	ThumbnailSize
};

const endpoints = makeApi([
	{
		method: 'get',
		path: '/api/albums',
		alias: 'getAllAlbums',
		requestFormat: 'json',
		response: z.array(Album)
	},
	{
		method: 'post',
		path: '/api/albums',
		alias: 'createAlbum',
		requestFormat: 'json',
		parameters: [
			{
				name: 'body',
				type: 'Body',
				schema: CreateAlbumRequest
			}
		],
		response: z.object({ albumId: z.number().int() }).passthrough()
	},
	{
		method: 'get',
		path: '/api/albums/:id',
		alias: 'getAlbumDetails',
		requestFormat: 'json',
		parameters: [
			{
				name: 'id',
				type: 'Path',
				schema: z.string()
			}
		],
		response: AlbumDetailsResponse
	},
	{
		method: 'put',
		path: '/api/albums/:id/assets',
		alias: 'appendAssetsToAlbum',
		requestFormat: 'json',
		parameters: [
			{
				name: 'body',
				type: 'Body',
				schema: AppendAssetsRequest
			},
			{
				name: 'id',
				type: 'Path',
				schema: z.string()
			}
		],
		response: z.object({ success: z.boolean() }).passthrough()
	},
	{
		method: 'get',
		path: '/api/asset',
		alias: 'getAllAssets',
		requestFormat: 'json',
		response: z.array(Asset)
	},
	{
		method: 'get',
		path: '/api/asset/:id',
		alias: 'getAsset',
		requestFormat: 'json',
		parameters: [
			{
				name: 'id',
				type: 'Path',
				schema: z.string()
			}
		],
		response: Asset,
		errors: [
			{
				status: 404,
				description: `Asset not found`,
				schema: z.void()
			}
		]
	},
	{
		method: 'get',
		path: '/api/asset/timeline',
		alias: 'getTimeline',
		requestFormat: 'json',
		parameters: [
			{
				name: 'lastAssetId',
				type: 'Query',
				schema: z.string().nullish()
			},
			{
				name: 'maxCount',
				type: 'Query',
				schema: z.number().int()
			},
			{
				name: 'lastFetch',
				type: 'Query',
				schema: z.string().nullish()
			}
		],
		response: TimelineChunk
	},
	{
		method: 'get',
		path: '/api/original/:id',
		alias: 'getAssetFile',
		requestFormat: 'json',
		parameters: [
			{
				name: 'id',
				type: 'Path',
				schema: z.string()
			}
		],
		response: z.void(),
		errors: [
			{
				status: 404,
				description: `Asset not found`,
				schema: z.void()
			}
		]
	},
	{
		method: 'get',
		path: '/api/repr/:assetId/:reprId',
		alias: 'getImageAssetRepresentation',
		requestFormat: 'json',
		parameters: [
			{
				name: 'assetId',
				type: 'Path',
				schema: z.string()
			},
			{
				name: 'reprId',
				type: 'Path',
				schema: z.string()
			}
		],
		response: z.void(),
		errors: [
			{
				status: 404,
				description: `Asset or Representation not found`,
				schema: z.void()
			}
		]
	},
	{
		method: 'get',
		path: '/api/thumbnail/:id/:size/:format',
		alias: 'getThumbnail',
		requestFormat: 'json',
		parameters: [
			{
				name: 'id',
				type: 'Path',
				schema: z.string()
			},
			{
				name: 'size',
				type: 'Path',
				schema: z.enum(['small', 'large'])
			},
			{
				name: 'format',
				type: 'Path',
				schema: z.enum(['avif', 'webp'])
			}
		],
		response: z.void()
	},
	{
		method: 'get',
		path: '/api/timeline/sections',
		alias: 'getTimelineSections',
		requestFormat: 'json',
		response: TimelineSectionsResponse
	},
	{
		method: 'get',
		path: '/api/timeline/sections/:id',
		alias: 'getTimelineSegments',
		requestFormat: 'json',
		parameters: [
			{
				name: 'id',
				type: 'Path',
				schema: z.string()
			}
		],
		response: TimelineSegmentsResponse
	}
]);

export const api = new Zodios(endpoints);

export function createApiClient(baseUrl: string, options?: ZodiosOptions) {
	return new Zodios(baseUrl, endpoints, options);
}
