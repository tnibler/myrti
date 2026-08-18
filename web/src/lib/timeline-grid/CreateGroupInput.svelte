<script lang="ts">
  import Button from '@lib/ui/Button.svelte';
  import type { ActionReturn } from 'svelte/action';
  import { onMount } from 'svelte';
  import { intl } from '@lib/i18next';

  type Props = {
    onSubmit: (title: string) => void;
    onCancel: () => void;
  };

  const { onSubmit, onCancel }: Props = $props();
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

<div class=" w-full flex flex-row">
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
