<script lang="ts">
  import type { ItemConfig } from "$shared/appData";
  import { Checkbox, Dialog, Separator } from "bits-ui";
  import { fade, scale } from "svelte/transition";

  type Props = {
    items: Readonly<ItemConfig[]>;
    selected: string[];

    onChangeSelect: (selected: string[]) => void;
  };

  let { items, selected, onChangeSelect }: Props = $props();

  const isAllSelected = $derived(selected.length === items.length);
  const selectedOptions = $derived(
    items.filter((sound) => selected.includes(sound.id))
  );

  const onSelectItem = (sound: ItemConfig) => {
    if (selected.includes(sound.id)) {
      onChangeSelect(selected.filter((id) => id !== sound.id));
    } else {
      onChangeSelect([...selected, sound.id]);
    }
  };

  const onToggleAll = () => {
    if (isAllSelected) {
      onChangeSelect([]);
    } else {
      onChangeSelect(items.map((sound) => sound.id));
    }
  };
</script>

<Dialog.Root>
  <Dialog.Trigger>Select Items</Dialog.Trigger>
  <Dialog.Portal>
    <Dialog.Overlay transition={fade} transitionConfig={{ duration: 150 }} />
    <Dialog.Content transition={scale}>
      <Dialog.Title>Select Items</Dialog.Title>

      <Dialog.Description class="text-sm text-foreground-alt">
        Choose which items will be thrown
      </Dialog.Description>

      <Separator.Root />

      <div class="throwable-table-wrapper">
        <table class="throwable-table">
          <thead>
            <tr>
              <th class="sound-column sound-column--checkbox">
                <Checkbox.Root
                  id="terms"
                  aria-labelledby="terms-label"
                  checked={isAllSelected}
                  onCheckedChange={onToggleAll}
                >
                  <Checkbox.Indicator let:isChecked>
                    {#if isChecked}
                      <span>&#10003;</span>
                    {/if}
                  </Checkbox.Indicator>
                </Checkbox.Root>
              </th>
              <th class="sound-column sound-column--preview">Preview</th>
              <th class="sound-column sound-column--name">Item Name</th>
            </tr>
          </thead>
          <tbody>
            {#each items as item (item.id)}
              <tr class="sound-row">
                <td class="sound-column sound-column--checkbox">
                  <Checkbox.Root
                    id="terms"
                    aria-labelledby="terms-label"
                    checked={selected.includes(item.id)}
                    onCheckedChange={() => onSelectItem(item)}
                  >
                    <Checkbox.Indicator let:isChecked>
                      {#if isChecked}
                        <span>&#10003;</span>
                      {/if}
                    </Checkbox.Indicator>
                  </Checkbox.Root>
                </td>

                <td class="sound-column sound-column--preview">
                  <div class="throwable__image-wrapper">
                    <img
                      class="throwable__image"
                      src={item.image.src}
                      alt="Throwable"
                    />
                  </div>
                </td>

                <td class="sound-column sound-column--name"> {item.name} </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div data-dialog-actions>
        <Dialog.Close>
          <span class="sr-only">Close</span>
        </Dialog.Close>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<div>Selected Items</div>

<ul>
  {#each selectedOptions as option}
    <li>{option.name}</li>
  {/each}
</ul>

<style>
  .throwable__image {
    width: 2.5rem;
    height: 2.5rem;
    object-fit: contain;
    background-color: #333;
    border-radius: 2rem;
  }

  .throwable-table-wrapper {
    padding: 1rem;
    max-height: 300px;
    overflow-y: auto;
    width: 100%;
  }

  .throwable-table {
    width: 100%;
    border-collapse: collapse;
  }

  .throwable-table tr {
    border: 1px solid #333;
  }

  .throwable-table thead {
    position: sticky;
    top: -25px;
    z-index: 1;
    background-color: #111;
  }

  .throwable-table td,
  .throwable-table th {
    padding: 0.5rem 0.25rem;
  }

  .throwable-table .sound-column--checkbox {
    padding-left: 1rem;
    padding-right: 0rem;
  }

  .throwable-table .sound-column--preview {
    padding-right: 1rem;
  }

  .throwable-table th {
    text-align: left;
    height: 2.5rem;
  }
</style>
