<script lang="ts">
  import type { TimelineGridItem } from './timeline.svelte';

  type Props = {
    className: string | undefined;
    timelineItem: TimelineGridItem & { type: 'segmentTitle' };
    onHeightTooSmall: (height: number) => void;
  };
  const { className, timelineItem, onHeightTooSmall }: Props = $props();
  let wantMinHeight = $derived(timelineItem.height);
  let actualHeight = $state(0);
  $effect(() => {
    if (actualHeight > wantMinHeight) {
      onHeightTooSmall(actualHeight);
    }
  });
</script>

<div
  bind:clientHeight={actualHeight}
  class={'absolute overflow-hidden whitespace-nowrap overflow-ellipsis' +
    ' ' +
    className +
    ' ' +
    (timelineItem.titleType === 'major' ? 'text-2xl' : 'text-lg')}
  style:top={timelineItem.top + 'px'}
  style:left={timelineItem.titleType === 'day' ? timelineItem.left + 'px' : undefined}
  style:width={timelineItem.titleType === 'day' ? timelineItem.width + 'px' : undefined}
>
  <!-- TODO:  fix the stuff above -->
  {timelineItem.title}
</div>
