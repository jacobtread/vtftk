<script lang="ts">
  import {
    createAppDateMutation,
    createDeleteSoundsMutation,
    getAppData,
  } from "$lib/api/runtimeAppData";
  import BulkSoundImport from "$lib/components/sounds/BulkSoundImport.svelte";
  import PageLayoutList from "$lib/layouts/PageLayoutList.svelte";
  import SoundItem from "$lib/sections/sounds/SoundItem.svelte";
  import type { SoundConfig } from "$shared/appData";
  import { Checkbox } from "bits-ui";
  import { toast } from "svelte-sonner";
  import DeleteIcon from "~icons/solar/trash-bin-2-bold";

  const appData = getAppData();
  const appDataMutation = createAppDateMutation();

  const deleteSounds = createDeleteSoundsMutation(appData, appDataMutation);

  let selected: string[] = $state([]);
  const isAllSelected = $derived(
    selected.length > 0 && selected.length === $appData.sounds.length
  );

  function onToggleSelected(item: SoundConfig) {
    if (selected.includes(item.id)) {
      selected = selected.filter((id) => id !== item.id);
    } else {
      selected = [...selected, item.id];
    }
  }

  function onToggleAllSelected() {
    if (isAllSelected) {
      selected = [];
    } else {
      selected = $appData.sounds.map((item) => item.id);
    }
  }

  function onBulkDelete() {
    if (!confirm("Are you sure you want to delete the selected sounds?")) {
      return;
    }

    const deletePromise = $deleteSounds(selected);

    toast.promise(deletePromise, {
      loading: "Deleting sounds...",
      success: "Deleted sounds",
      error: "Failed to delete sounds",
    });

    selected = [];
  }
</script>

{#snippet actions()}
  <a class="btn" href="/sounds/create"> Create Sound </a>
  <BulkSoundImport />
{/snippet}

{#snippet beforeContent()}
  <div class="selection">
    <Checkbox.Root
      checked={isAllSelected}
      onCheckedChange={onToggleAllSelected}
    >
      <Checkbox.Indicator let:isChecked>
        {#if isChecked}
          <span>&#10003;</span>
        {/if}
      </Checkbox.Indicator>
    </Checkbox.Root>

    {#if selected.length > 0}
      <div class="selection__count">
        {selected.length} Selected
      </div>

      <div class="selection__actions">
        <button class="btn" onclick={onBulkDelete}><DeleteIcon /> Delete</button
        >
      </div>
    {/if}
  </div>
{/snippet}

<PageLayoutList
  title="Sounds"
  description="Create sounds that can be triggered"
  {actions}
  {beforeContent}
>
  <div class="grid">
    {#each $appData.sounds as sound}
      <SoundItem
        config={sound}
        selected={selected.includes(sound.id)}
        onToggleSelected={() => onToggleSelected(sound)}
      />
    {/each}
  </div>
</PageLayoutList>

<style>
  .selection {
    display: flex;
    align-items: center;
    gap: 1rem;
    height: 3rem;
    flex-shrink: 0;
  }

  .selection__count {
    flex: auto;
  }

  .selection__actions {
    display: flex;
    gap: 1rem;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    width: 100%;
  }
</style>
