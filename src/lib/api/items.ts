import type {
  CreateItem,
  Item,
  ItemId,
  ItemWithImpactSounds,
  Sound,
  SoundId,
  UpdateItem,
} from "$shared/dataV2";
import { createMutation, createQuery } from "@tanstack/svelte-query";
import { invoke } from "@tauri-apps/api/core";
import { queryClient } from "./utils";
import { derived, type Readable } from "svelte/store";

const ITEMS_KEY = ["items"];

export function createItemsQuery() {
  return createQuery({
    queryKey: ITEMS_KEY,
    queryFn: () => invoke<Item[]>("get_items"),
  });
}

function createItemKey(id: ItemId) {
  return ["item", id] as const;
}

export function createItemQuery(id: ItemId) {
  return createQuery({
    queryKey: createItemKey(id),
    queryFn: () => getItemById(id),
  });
}
export function createItemQueryDerived(id: Readable<ItemId>) {
  return createQuery(
    derived(id, (id) => ({
      queryKey: createItemKey(id),
      queryFn: () => getItemById(id),
    }))
  );
}

export function createItemMutation() {
  return createMutation<Item, Error, CreateItem>({
    mutationFn: (createItem) =>
      invoke<Item>("create_item", { create: createItem }),

    onSuccess: (data) => {
      // Invalidate the specific item query
      const itemKey = createItemKey(data.id);
      queryClient.setQueryData(itemKey, data);
    },
    onSettled: (_data, _err, _createItem) => {
      // Invalid the list of items
      queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
    },
  });
}

export function bulkCreateItemMutation() {
  return createMutation<Item[], Error, CreateItem[]>({
    mutationFn: (createItems) =>
      Promise.all(
        createItems.map((createItem) =>
          invoke<Item>("create_item", { create: createItem })
        )
      ),

    onSuccess: (items) => {
      for (const item of items) {
        // Invalidate the specific item query
        const itemKey = createItemKey(item.id);
        queryClient.setQueryData(itemKey, item);
      }
    },
    onSettled: (_data, _err, _createItem) => {
      // Invalid the list of items
      queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
    },
  });
}

export function getItemSounds(itemId: ItemId) {
  return invoke<Sound[]>("get_item_sounds", { itemId });
}

export function getItemById(itemId: ItemId) {
  return invoke<ItemWithImpactSounds | null>("get_item_by_id", { itemId });
}

export function updateItemMutation() {
  return createMutation<Item, Error, UpdateItem>({
    mutationFn: (updateItem) =>
      invoke<Item>("update_item", {
        itemId: updateItem.itemId,
        update: updateItem.update,
      }),
    onSuccess: (data) => {
      // Invalidate the specific item query
      const itemKey = createItemKey(data.id);
      queryClient.setQueryData(itemKey, data);
    },
    onSettled: (_data, _err, _updateItem) => {
      // Invalid the list of items
      queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
    },
  });
}

export function deleteItemMutation() {
  return createMutation<void, Error, ItemId>({
    mutationFn: (itemId) => invoke<void>("delete_item", { itemId }),
    onMutate: async (itemId) => {
      const itemKey = createItemKey(itemId);

      // Cancel any queries for the item and clear the current item data
      queryClient.cancelQueries({ queryKey: itemKey });
      queryClient.setQueryData(itemKey, undefined);

      return undefined;
    },
    onSettled: (_data, _err, itemId) => {
      // Invalidate the specific item query
      const itemKey = createItemKey(itemId);
      queryClient.invalidateQueries({ queryKey: itemKey });

      // Invalid the list of items
      queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
    },
  });
}

type BulkAppendItemSounds = {
  itemIds: ItemId[];
  soundIds: SoundId[];
};

export function bulkAppendItemSoundsMutation() {
  return createMutation<void[], Error, BulkAppendItemSounds>({
    mutationFn: (bulkAppendSounds) => {
      return Promise.all(
        bulkAppendSounds.itemIds.map((itemId) =>
          invoke<void>("append_item_impact_sounds", {
            itemId,
            sounds: bulkAppendSounds.soundIds,
          })
        )
      );
    },
    onMutate: async (bulkAppendSounds) => {
      for (const itemId of bulkAppendSounds.itemIds) {
        const itemKey = createItemKey(itemId);
        // Cancel any queries for the item and clear the current item data
        queryClient.cancelQueries({ queryKey: itemKey });
        queryClient.setQueryData(itemKey, undefined);
      }

      return undefined;
    },
    onSettled: (_data, _err, bulkAppendSounds) => {
      for (const itemId of bulkAppendSounds.itemIds) {
        // Invalidate the specific item query
        const itemKey = createItemKey(itemId);
        queryClient.invalidateQueries({ queryKey: itemKey });
      }

      // Invalid the list of items
      queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
    },
  });
}

type BulkDeleteItems = {
  itemIds: ItemId[];
};

export function bulkDeleteItemsMutation() {
  return createMutation<void[], Error, BulkDeleteItems>({
    mutationFn: (deleteItems) => {
      return Promise.all(
        deleteItems.itemIds.map((itemId) =>
          invoke<void>("delete_item", { itemId })
        )
      );
    },
    onMutate: async (deleteItems) => {
      for (const itemId of deleteItems.itemIds) {
        const itemKey = createItemKey(itemId);

        // Cancel any queries for the item and clear the current item data
        queryClient.cancelQueries({ queryKey: itemKey });
        queryClient.setQueryData(itemKey, undefined);
      }

      return undefined;
    },
    onSettled: (_data, _err, deleteItems) => {
      for (const itemId of deleteItems.itemIds) {
        // Invalidate the specific item query
        const itemKey = createItemKey(itemId);
        queryClient.invalidateQueries({ queryKey: itemKey });

        // Invalid the list of items
        queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
      }

      // Invalid the list of items
      queryClient.invalidateQueries({ queryKey: ITEMS_KEY });
    },
  });
}