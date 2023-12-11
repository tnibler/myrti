import { makeApi, Zodios, type ZodiosOptions } from '@zodios/core';
import { z } from 'zod';

const AssetRootDirId = z.string();
const AssetId = z.string();
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
		pathInRoot: z.string(),
		takenDate: z.string().datetime({ offset: true }),
		type: AssetType,
		width: z.number().int()
	})
	.passthrough();
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
const Video = z.object({}).partial().passthrough();
const AssetSpe = z.union([Image, Video]);
const AssetWithSpe = Asset.and(AssetSpe).and(z.object({}).partial().passthrough());
const TimelineGroupType = z.union([
	z.object({ day: z.string().datetime({ offset: true }) }).passthrough(),
	z
		.object({
			group: z
				.object({
					end: z.string().datetime({ offset: true }),
					start: z.string().datetime({ offset: true }),
					title: z.string()
				})
				.passthrough()
		})
		.passthrough()
]);
const TimelineGroup = z
	.object({ assets: z.array(AssetWithSpe), type: TimelineGroupType })
	.passthrough();
const TimelineChunk = z
	.object({
		changedSinceLastFetch: z.boolean(),
		date: z.string().datetime({ offset: true }),
		groups: z.array(TimelineGroup)
	})
	.passthrough();
const AlbumId = z.string();
const ThumbnailFormat = z.enum(['avif', 'webp']);
const ThumbnailSize = z.enum(['small', 'large']);

export const schemas = {
	AssetRootDirId,
	AssetId,
	AssetMetadataType,
	AssetMetadata,
	AssetType,
	Asset,
	ImageRepresentation,
	Image,
	Video,
	AssetSpe,
	AssetWithSpe,
	TimelineGroupType,
	TimelineGroup,
	TimelineChunk,
	AlbumId,
	ThumbnailFormat,
	ThumbnailSize
};

const endpoints = makeApi([
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
	}
]);

export const api = new Zodios(endpoints);

export function createApiClient(baseUrl: string, options?: ZodiosOptions) {
	return new Zodios(baseUrl, endpoints, options);
}
