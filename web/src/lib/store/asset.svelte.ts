import type { z } from 'zod'
import { schemas, api } from '../apiclient'

type AssetStore = {
  groups: z.infer<typeof schemas.TimelineGroup>[],
  lastAssetId: string | null
}

export function createAssetStore() {
  const store: AssetStore = $state({
    groups: [],
    lastAssetId: null
  })
  const assetGroups = $derived(store.groups)

  return {
    assetGroups,
    fetchMore: async () => {
      const chunk = await api.getTimeline({
        queries: {
          lastAssetId: store.lastAssetId,
          lastFetch: "",
          maxCount: 3
        }
      })
      const lastGroup = chunk.groups[chunk.groups.length - 1]
      if (lastGroup) {
        const lastAsset = lastGroup.assets[lastGroup.assets.length - 1]
        store.lastAssetId = lastAsset.id
      }
      store.groups.push(...chunk.groups)
    }
  }
}
