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

export const schemas = {
	AssetRootDirId,
	AssetId,
	AssetMetadataType,
	AssetMetadata,
	AssetType,
	Asset
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
