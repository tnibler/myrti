<script lang="ts">
  import type { AssetWithSpe } from '@api/myrti';
  import { dayjs } from '@lib/dayjs';
  import { disableGhiStreaming, getFileDetails, regenerateThumbnail } from '../../api/myrti';
  import { getFileDetailsResponse } from '../../api/myrti.zod';
  import Button from '@lib/ui/Button.svelte';

  type Props = {
    asset: AssetWithSpe;
  };

  const { asset }: Props = $props();
  const assetMetadata = $derived.by(async () => {
    const resp = await getFileDetails(asset.repFile.fileId);
    const result = getFileDetailsResponse.parse(resp.data);
    const entries: [string, unknown][] = [];
    for (const [group, groupEntry] of Object.entries(result.exiftoolOutput)) {
      if (groupEntry !== null && typeof groupEntry === 'object' && !Array.isArray(groupEntry)) {
        entries.push(...Object.entries(groupEntry));
      } else {
        entries.push([group, groupEntry]);
      }
    }
    return entries;
  });
</script>

<div class="overflow-y-scroll px-2 h-full flex flex-col">
  {#await assetMetadata then entries}
    <ul>
      <li>
        {asset.repFile.pathInRoot}
      </li>

      <li>
        {dayjs.utc(asset.takenDate).format('LLL')}
      </li>
      {#each entries as [key, value]}
        <li>{key}: {value}</li>
      {/each}
    </ul>
  {/await}
  <Button
    text="Regenerate thumbnails"
    onclick={async (e) => {
      e.target.disabled = true;
      try {
        await regenerateThumbnail({ fileIds: [asset.repFile.fileId] });
      } finally {
        e.target.disabled = false;
      }
    }}
  />
  {#if asset.assetType === 'video'}
    <Button
      text="Video doesn't play right"
      onclick={async (e) => {
        e.target.disabled = true;
        try {
          await disableGhiStreaming({ fileIds: [asset.repFile.fileId] });
        } finally {
          e.target.disabled = false;
        }
      }}
    />
  {/if}
</div>
