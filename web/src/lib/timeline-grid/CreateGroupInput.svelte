<script lang="ts">
  import Button from '@lib/ui/Button.svelte';
  import type { TimelineGridItem } from './timeline.svelte';
  import type { ActionReturn } from 'svelte/action';
  import { onMount } from 'svelte';
  import { intl } from '@lib/i18next';

  type Props = {
    item: TimelineGridItem & { type: 'createGroupTitleInput' };
    onSubmit: (title: string) => void;
    onCancel: () => void;
  };

  const { item, onSubmit, onCancel }: Props = $props();
  let input: HTMLInputElement | null = null;

  onMount(() => {
    input?.focus();
  });

  function trySubmit() {
    const title = input?.value?.trim();
    if (title !== undefined && title.length > 0) {
      onSubmit(title);
    }
  }

  function inputKeyBinds(el: HTMLInputElement): ActionReturn {
    el.onkeyup = (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCancel();
      } else if (e.key === 'Enter') {
        trySubmit();
      }
    };
    return {
      destroy: () => {
        el.onkeyup = null;
      },
    };
  }

  // TODO: disable button if title input is empty
</script>

<div class="absolute w-full flex-row" style="top: {item.top}px;">
  <input use:inputKeyBinds placeholder={intl('group_title_placeholder')} bind:this={input} />
  <Button
    text={intl('ok')}
    onclick={() => {
      trySubmit();
    }}
  />
  <Button
    text={intl('cancel')}
    onclick={() => {
      onCancel();
    }}
  />
</div>
