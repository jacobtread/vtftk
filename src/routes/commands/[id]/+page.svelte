<script lang="ts">
  import { page } from "$app/stores";
  import { getAppData } from "$lib/api/runtimeAppData";
  import type { CommandConfig } from "$lib/api/types";
  import PageLayoutList from "$lib/layouts/PageLayoutList.svelte";
  import CommandForm from "$lib/sections/commands/CommandForm.svelte";
  import { derived, type Readable } from "svelte/store";

  const appData = getAppData();

  const item: Readable<CommandConfig | undefined> = derived(
    [appData, page],
    ([$appData, $page]) => {
      const id = $page.params.id;
      const item = $appData.commands.find((item) => item.id === id);
      return item;
    }
  );
</script>

{#if $item !== undefined}
  <CommandForm existing={$item} />
{:else}
  {#snippet actions()}
    <a type="button" href="/scripts">Back</a>
  {/snippet}

  <PageLayoutList
    title="Script Not Found"
    description="Unknown script"
    {actions}
  ></PageLayoutList>
{/if}
