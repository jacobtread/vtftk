<script lang="ts">
  import { page } from "$app/stores";
  import { createScriptQuery } from "$lib/api/scripts";
  import PageLayoutList from "$lib/layouts/PageLayoutList.svelte";
  import ScriptForm from "$lib/sections/scripts/ScriptForm.svelte";

  const id = $derived($page.params.id);
  const scriptQuery = $derived(createScriptQuery(id));
</script>

{#if $scriptQuery.isLoading}
  Loading...
{:else if $scriptQuery.data}
  <ScriptForm existing={$scriptQuery.data} />
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
