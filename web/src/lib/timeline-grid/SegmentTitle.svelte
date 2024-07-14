<script lang="ts">
  import type { TimelineGridItem } from './timeline.svelte';

  type Props = {
    className: string | undefined;
    timelineItem: TimelineGridItem & { type: 'segmentTitle' };
    setActualHeight: (height: number) => void;
  };
  const { className, timelineItem, setActualHeight }: Props = $props();
  let height = $state(timelineItem.height);
  $effect(() => {
    if (height != timelineItem.height) {
      setActualHeight(height);
    }
  });
</script>

<div
  bind:clientHeight={height}
  class="absolute text-2xl {className}"
  style:top={timelineItem.top + 'px'}
>
  <!-- style:height={timelineItem.height + 'px'} -->
  {timelineItem.title}
</div>
