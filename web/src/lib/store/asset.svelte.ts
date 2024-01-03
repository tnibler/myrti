import { api } from '../apiclient';
import type { TimelineGroup } from '$lib/apitypes';

type AssetStore = {
	groups: TimelineGroup[];
	// used for paging to fetch next chunk of assets
	lastAssetId: string | null;
};

export function createAssetStore() {
	const store: AssetStore = $state({
		groups: [],
		lastAssetId: null
	});
	const assetGroups = $derived(store.groups);

	return {
		assetGroups,
		fetchMore: async () => {
			const chunk = await api.getTimeline({
				queries: {
					lastAssetId: store.lastAssetId,
					lastFetch: '',
					maxCount: 10
				}
			});
			const lastGroup = chunk.groups[chunk.groups.length - 1];
			if (lastGroup) {
				const lastAsset = lastGroup.assets[lastGroup.assets.length - 1];
				store.lastAssetId = lastAsset.id;
			}

			// check if last TimelineGroup and first of new chunk need to be merged
			const currentLastGroup = store.groups[store.groups.length - 1];
			const newFirstGroup = chunk.groups[0];
			let mergeLastAndFirst = false;
			if (currentLastGroup && newFirstGroup) {
				if (
					newFirstGroup.type === 'day' &&
					currentLastGroup.type === 'day' &&
					newFirstGroup.date === currentLastGroup.date
				) {
					// merge two chunked days
					mergeLastAndFirst = true;
				} else if (
					newFirstGroup.type === 'group' &&
					currentLastGroup.type === 'group' &&
					newFirstGroup.groupId == currentLastGroup.groupId
				) {
					// merge two chunked groups
					mergeLastAndFirst = true;
				}
			}
			if (mergeLastAndFirst) {
				currentLastGroup.assets.push(...newFirstGroup.assets);
				store.groups.push(...chunk.groups.slice(1, chunk.groups.length));
			} else {
				store.groups.push(...chunk.groups);
			}
		}
	};
}
