<script lang="ts">
  import Button from '@lib/ui/Button.svelte';
  import type { TimelineGridItem } from './timeline.svelte';

  type Props = {
    item: TimelineGridItem & { type: 'createGroupTitleInput' };
    onSubmit: (title: string) => void;
    onCancel: () => void;
  };

  const { item, onSubmit, onCancel }: Props = $props();
  let input: HTMLInputElement | null = null;

  // TODO: enter/escape to confirm/cancel create title field
  // TODO: disable button if title input is empty
</script>

<div class="absolute flex-row" style="top: {item.top}px; left: {item.left}px;">
  <input placeholder="Enter group title" bind:this={input} />
  <Button
    text="Ok"
    onclick={() => {
      const title = input?.value;
      if (title === undefined) {
        return;
      }
      onSubmit(title);
    }}
  />
  <Button
    text="Cancel"
    onclick={() => {
      onCancel();
    }}
  />
</div>
