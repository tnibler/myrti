<script lang="ts">
  import { api } from '@lib/apiclient';
  import type { AssetWithSpe } from '@lib/apitypes';
  import { dayjs } from '@lib/dayjs';

  type Props = {
    asset: AssetWithSpe;
  };

  const { asset }: Props = $props();
  const assetMetadata = api.getAssetDetails({ params: { id: asset.id } }).then((resp) => {
    const entries: [string, unknown][] = [];
    for (const [group, groupEntry] of Object.entries(resp.exiftoolOutput)) {
      if (groupEntry !== null && typeof groupEntry === 'object' && !Array.isArray(groupEntry)) {
        entries.push(...Object.entries(groupEntry));
      } else {
        entries.push([group, groupEntry]);
      }
    }
    return entries;
  });
</script>

<div class="overflow-y-scroll px-2 h-full">
  {#await assetMetadata then entries}
    <ul>
      <li>
        {asset.pathInRoot}
      </li>

      <li>
        {dayjs.utc(asset.takenDate).format('LLL')}
      </li>
      {#each entries as [key, value]}
        <li>{key}: {value}</li>
      {/each}
    </ul>
  {/await}
</div>
