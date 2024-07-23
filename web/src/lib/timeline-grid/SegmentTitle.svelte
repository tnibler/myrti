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
  class={'absolute {className} ' + (timelineItem.titleType === 'major' ? 'text-2xl' : 'text-lg')}
  style:white-space="nowrap"
  style:overflow="hidden"
  style:text-overflow="ellipsis"
  style:top={timelineItem.top + 'px'}
  style:height={timelineItem.height + 'px'}
  style:left={timelineItem.titleType === 'day' ? timelineItem.left + 'px' : undefined}
  style:width={timelineItem.titleType === 'day' ? timelineItem.width + 'px' : undefined}
>
  <!-- TODO:  fix the stuff above -->
  {timelineItem.title}
</div>
