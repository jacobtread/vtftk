<script lang="ts">
  import { createGetRedeemsList, refreshRedeemsList } from "$lib/api/twitch";
  import FormSelect from "$lib/components/form/FormSelect.svelte";
  import { derived } from "svelte/store";
  import SolarRefreshBold from "~icons/solar/refresh-bold";

  type Props = {
    id: string;
    name: string;
    label: string;

    selected: any;
    onChangeSelected: (value: any) => void;
  };

  const { id, label, name, selected, onChangeSelected }: Props = $props();

  const redeemsList = createGetRedeemsList();

  const options = derived(redeemsList, ($redeemsList) =>
    ($redeemsList.data ?? []).map((item) => ({
      value: item.id,
      label: item.title,
      description: item.prompt,
    }))
  );
</script>

{#snippet twitchRedeemItem(item: any)}
  <div class="text-stack">
    <p class="text-stack--top">{item.label}</p>
    <p class="text-stack--bottom">{item.description}</p>
  </div>
{/snippet}

<div class="container">
  <FormSelect
    {id}
    {name}
    {label}
    items={$options}
    item={twitchRedeemItem}
    {selected}
    {onChangeSelected}
  />

  <button type="button" class="btn" onclick={refreshRedeemsList}>
    <SolarRefreshBold />
    Refresh Redeems
  </button>
</div>

{#if $redeemsList.isLoading}
  Loading...
{/if}

<style>
  .container {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
    width: 100%;
  }
</style>
