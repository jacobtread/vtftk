<script lang="ts">
  import { createForm } from "felte";
  import { validator } from "@felte/validator-zod";
  import reporterDom from "@felte/reporter-dom";
  import { z } from "zod";

  import {
    BitsAmountType,
    EventOutcomeType,
    EventTriggerType,
    MINIMUM_REQUIRED_ROLE_VALUES,
    MinimumRequiredRole,
    ThrowableDataType,
  } from "$shared/appData";
  import FormTextInput from "$lib/components/form/FormTextInput.svelte";
  import FormNumberInput from "$lib/components/form/FormNumberInput.svelte";
  import TwitchRedeemSelect from "../twitch/TwitchRedeemSelect.svelte";
  import HotkeySelect from "./HotkeySelect.svelte";
  import { goto } from "$app/navigation";
  import ThrowablePicker from "$lib/components/throwable/ThrowablePicker.svelte";
  import PageLayoutList from "$lib/layouts/PageLayoutList.svelte";
  import EventTriggerTypeSelect from "./EventTriggerTypeSelect.svelte";
  import RequiredRoleSelect from "./RequiredRoleSelect.svelte";
  import OutcomeTypeSelect from "./OutcomeTypeSelect.svelte";
  import ThrowableDataTypeSelect from "./ThrowableDataTypeSelect.svelte";
  import FormSection from "$lib/components/form/FormSection.svelte";
  import FormBoundCheckbox from "$lib/components/form/FormBoundCheckbox.svelte";
  import { toast } from "svelte-sonner";
  import SoundSelect from "./SoundSelect.svelte";
  import { createEventMutation, updateEventMutation } from "$lib/api/vevents";
  import type { VEvent } from "$shared/dataV2";
  import { Tabs } from "bits-ui";
  import SolarBookBoldDuotone from "~icons/solar/book-bold-duotone";
  import SolarCardReciveBoldDuotone from "~icons/solar/card-recive-bold-duotone";
  import SolarCardSendBoldDuotone from "~icons/solar/card-send-bold-duotone";
  import SolarChecklistMinimalisticBoldDuotone from "~icons/solar/checklist-minimalistic-bold-duotone";

  type Props = {
    existing?: VEvent;
  };

  const { existing }: Props = $props();

  const updateEvent = updateEventMutation();
  const createEvent = createEventMutation();

  const triggerSchema = z.discriminatedUnion("type", [
    z.object({
      type: z.literal(EventTriggerType.Redeem),
      reward_id: z.string(),
    }),
    z.object({
      type: z.literal(EventTriggerType.Command),
      message: z.string(),
    }),
    z.object({
      type: z.literal(EventTriggerType.Follow),
    }),
    z.object({
      type: z.literal(EventTriggerType.Subscription),
    }),
    z.object({
      type: z.literal(EventTriggerType.GiftedSubscription),
    }),
    z.object({
      type: z.literal(EventTriggerType.Bits),
      min_bits: z.number(),
    }),
    z.object({
      type: z.literal(EventTriggerType.Raid),
      min_raiders: z.number(),
    }),
  ]);

  type TriggerSchema = z.infer<typeof triggerSchema>;

  const throwableDataSchema = z.discriminatedUnion("type", [
    z.object({
      type: z.literal(ThrowableDataType.Throw),
      throwable_ids: z.array(z.string()),
      amount: z.number(),
    }),
    z.object({
      type: z.literal(ThrowableDataType.Barrage),
      throwable_ids: z.array(z.string()),
      amount_per_throw: z.number(),
      frequency: z.number(),
      amount: z.number(),
    }),
  ]);

  type ThrowableDataSchema = z.infer<typeof throwableDataSchema>;

  const outcomeSchema = z.discriminatedUnion("type", [
    z.object({
      type: z.literal(EventOutcomeType.ThrowBits),
      _1: z.string().nullable(),
      _100: z.string().nullable(),
      _1000: z.string().nullable(),
      _5000: z.string().nullable(),
      _10000: z.string().nullable(),
      amount: z.discriminatedUnion("type", [
        z.object({ type: z.literal(BitsAmountType.Fixed), amount: z.number() }),
        z.object({
          type: z.literal(BitsAmountType.Dynamic),
          max_amount: z.number(),
        }),
      ]),
    }),
    z.object({
      type: z.literal(EventOutcomeType.Throwable),
      data: throwableDataSchema,
    }),

    z.object({
      type: z.literal(EventOutcomeType.TriggerHotkey),
      hotkey_id: z.string(),
    }),
    z.object({
      type: z.literal(EventOutcomeType.PlaySound),
      sound_id: z.string(),
    }),
  ]);

  type OutcomeSchema = z.infer<typeof outcomeSchema>;

  const schema = z.object({
    name: z.string().min(1, "Name is required"),
    enabled: z.boolean(),

    trigger: triggerSchema,
    outcome: outcomeSchema,

    require_role: z.enum(MINIMUM_REQUIRED_ROLE_VALUES),
    cooldown: z.number(),
    outcome_delay: z.number(),
  });

  type Schema = z.infer<typeof schema>;

  const createDefaults: Schema = {
    name: "",
    enabled: true,
    trigger: {
      type: EventTriggerType.Redeem,
      reward_id: "",
    },
    outcome: {
      type: EventOutcomeType.Throwable,
      data: {
        type: ThrowableDataType.Throw,
        throwable_ids: [],
        amount: 1,
      },
    },
    require_role: MinimumRequiredRole.None,
    cooldown: 0,
    outcome_delay: 0,
  };

  function createFromExisting(config: VEvent): Partial<Schema> {
    return {
      ...config,
    };
  }

  const { form, data, setFields } = createForm<Schema>({
    // Derive initial values
    initialValues: existing ? createFromExisting(existing) : createDefaults,

    // Validation and error reporting
    extend: [validator({ schema }), reporterDom()],

    onSubmit(values) {
      const savePromise = save(values);

      toast.promise(
        savePromise,
        existing
          ? {
              loading: "Saving event...",
              success: "Saved event",
              error: "Failed to save event",
            }
          : {
              loading: "Creating event...",
              success: "Created event",
              error: "Failed to create event",
            }
      );

      if (!existing) {
        goto("/events");
      }
    },
  });

  async function save(values: Schema) {
    if (existing) {
      await $updateEvent.mutateAsync({
        eventId: existing.id,
        update: {
          name: values.name,
          enabled: values.enabled,
          trigger: values.trigger,
          outcome: values.outcome,
          cooldown: values.cooldown,
          require_role: values.require_role,
          outcome_delay: values.outcome_delay,
        },
      });
    } else {
      await $createEvent.mutateAsync({
        name: values.name,
        enabled: values.enabled,
        trigger: values.trigger,
        outcome: values.outcome,
        cooldown: values.cooldown,
        require_role: values.require_role,
        outcome_delay: values.outcome_delay,
      });
    }
  }

  function getTriggerDefaults(type: EventTriggerType): TriggerSchema {
    switch (type) {
      case EventTriggerType.Redeem:
        return { type: EventTriggerType.Redeem, reward_id: "" };
      case EventTriggerType.Command:
        return { type: EventTriggerType.Command, message: "" };
      case EventTriggerType.Follow:
        return { type: EventTriggerType.Follow };
      case EventTriggerType.Subscription:
        return { type: EventTriggerType.Subscription };
      case EventTriggerType.GiftedSubscription:
        return { type: EventTriggerType.GiftedSubscription };
      case EventTriggerType.Bits:
        return { type: EventTriggerType.Bits, min_bits: 1 };
      case EventTriggerType.Raid:
        return { type: EventTriggerType.Raid, min_raiders: 1 };
    }
  }

  function getOutcomeDefaults(type: EventOutcomeType): OutcomeSchema {
    switch (type) {
      case EventOutcomeType.ThrowBits:
        return {
          type: EventOutcomeType.ThrowBits,
          _1: null,
          _100: null,
          _1000: null,
          _10000: null,
          _5000: null,
          amount: {
            type: BitsAmountType.Dynamic,
            max_amount: 20,
          },
        };
      case EventOutcomeType.Throwable:
        return {
          type: EventOutcomeType.Throwable,
          data: {
            type: ThrowableDataType.Throw,
            amount: 1,
            throwable_ids: [],
          },
        };
      case EventOutcomeType.TriggerHotkey:
        return {
          type: EventOutcomeType.TriggerHotkey,
          hotkey_id: "",
        };
      case EventOutcomeType.PlaySound:
        return {
          type: EventOutcomeType.PlaySound,
          sound_id: "",
        };
    }
  }

  function onChangeTriggerType(type: EventTriggerType) {
    const defaults = getTriggerDefaults(type);

    // Reset invalid outcomes
    if (
      type !== EventTriggerType.Bits &&
      $data.outcome.type === EventOutcomeType.ThrowBits
    ) {
      setFields(
        "outcome",
        getOutcomeDefaults(EventOutcomeType.Throwable),
        true
      );
    }

    setFields("trigger", defaults, true);
  }

  function onChangeOutcomeType(type: EventOutcomeType) {
    const defaults = getOutcomeDefaults(type);
    setFields("outcome", defaults, true);
  }

  function getThrowableDataDefaults(
    type: ThrowableDataType
  ): ThrowableDataSchema {
    switch (type) {
      case ThrowableDataType.Throw:
        return { type: ThrowableDataType.Throw, amount: 1, throwable_ids: [] };
      case ThrowableDataType.Barrage:
        return {
          type: ThrowableDataType.Barrage,
          amount: 1,
          amount_per_throw: 1,
          frequency: 100,
          throwable_ids: [],
        };
    }
  }

  function onChangeThrowableDataType(type: ThrowableDataType) {
    const defaults = getThrowableDataDefaults(type);
    setFields("outcome.data", defaults, true);
  }
</script>

<form use:form>
  {#snippet actions()}
    <button type="submit" class="btn">{existing ? "Save" : "Create"}</button>
    <a type="button" class="btn" href="/events">Back</a>
  {/snippet}

  <PageLayoutList
    title={existing ? "Edit Event" : "Create Event"}
    description={existing
      ? "Editing Event"
      : "Create an event that will trigger some outcome"}
    {actions}
  >
    <div class="content">
      <Tabs.Root>
        <Tabs.List>
          <Tabs.Trigger value="details">
            <SolarBookBoldDuotone />
            Details
          </Tabs.Trigger>
          <Tabs.Trigger value="trigger">
            <SolarCardReciveBoldDuotone />
            Trigger
          </Tabs.Trigger>
          <Tabs.Trigger value="outcome">
            <SolarCardSendBoldDuotone />
            Outcome
          </Tabs.Trigger>
          <Tabs.Trigger value="cooldown">
            <SolarChecklistMinimalisticBoldDuotone />
            Requirements
          </Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="details">
          <!-- Base options -->
          <FormSection
            title="Details"
            description="Basic details about the event"
          >
            <FormTextInput id="name" name="name" label="Name" />
            <FormBoundCheckbox
              id="enabled"
              name="enabled"
              label="Enabled"
              description="Whether this event can be triggered"
            />
          </FormSection>
        </Tabs.Content>
        <Tabs.Content value="trigger">
          <!-- Trigger options -->
          <FormSection
            title="Trigger"
            description="What should trigger this event"
          >
            <EventTriggerTypeSelect
              id="trigger.type"
              name="trigger.type"
              label="Event Trigger"
              selected={$data.trigger.type}
              onChangeSelected={(selected) => {
                onChangeTriggerType(selected);
              }}
            />

            {#if $data.trigger.type === EventTriggerType.Redeem}
              <TwitchRedeemSelect
                id="trigger.reward_id"
                name="trigger.reward_id"
                label="Reward"
                selected={$data.trigger.reward_id}
                onChangeSelected={(selected) =>
                  setFields("trigger.reward_id", selected, true)}
              />
            {:else if $data.trigger.type === EventTriggerType.Command}
              <FormTextInput
                id="trigger.message"
                name="trigger.message"
                label="Command Message"
              />
            {:else if $data.trigger.type === EventTriggerType.Bits}
              <FormNumberInput
                id="trigger.min_bits"
                name="trigger.min_bits"
                label="Minimum Bits"
              />
            {:else if $data.trigger.type === EventTriggerType.Raid}
              <FormNumberInput
                id="trigger.min_raiders"
                name="trigger.min_raiders"
                label="Minimum Raiders"
              />
            {/if}
          </FormSection>
        </Tabs.Content>
        <Tabs.Content value="outcome">
          <!-- Outcome options -->
          <FormSection
            title="Outcome"
            description="What should happen when this event is triggered"
          >
            <OutcomeTypeSelect
              id="outcome.type"
              name="outcome.type"
              label="Event Outcome"
              triggerType={$data.trigger.type}
              selected={$data.outcome.type}
              onChangeSelected={(selected) => {
                onChangeOutcomeType(selected);
              }}
            />

            {#if $data.outcome.type === EventOutcomeType.ThrowBits}
              <div></div>
            {:else if $data.outcome.type === EventOutcomeType.Throwable}
              <ThrowableDataTypeSelect
                id="outcome.data.type"
                name="outcome.data.type"
                label="Throwable Type"
                selected={$data.outcome.data.type}
                onChangeSelected={(selected) => {
                  onChangeThrowableDataType(selected);
                }}
              />

              {#if $data.outcome.data.type === ThrowableDataType.Throw}
                <FormNumberInput
                  id="outcome.data.amount"
                  name="outcome.data.amount"
                  label="Total number of items to throw"
                />
                <ThrowablePicker
                  selected={$data.outcome.data.throwable_ids}
                  onChangeSelect={(selected) => {
                    setFields("outcome.data.throwable_ids", selected, true);
                  }}
                />
              {:else if $data.outcome.data.type === ThrowableDataType.Barrage}
                <div>
                  <ThrowablePicker
                    selected={$data.outcome.data.throwable_ids}
                    onChangeSelect={(selected) => {
                      setFields("outcome.data.throwable_ids", selected, true);
                    }}
                  />

                  <div class="throwable-config-grid">
                    <FormNumberInput
                      id="outcome.data.amount_per_throw"
                      name="outcome.data.amount_per_throw"
                      label="Amount of items per throw"
                    />

                    <FormNumberInput
                      id="outcome.data.frequency"
                      name="outcome.data.frequency"
                      label="Frequency"
                    />

                    <FormNumberInput
                      id="outcome.data.amount"
                      name="outcome.data.amount"
                      label="Total number of throws"
                    />
                  </div>
                </div>
              {/if}
            {:else if $data.outcome.type === EventOutcomeType.TriggerHotkey}
              <HotkeySelect
                id="outcome.hotkey_id"
                name="outcome.hotkey_id"
                label="Hotkey"
                selected={$data.outcome.hotkey_id}
                onChangeSelected={(selected) =>
                  setFields("outcome.hotkey_id", selected, true)}
              />
            {:else if $data.outcome.type === EventOutcomeType.PlaySound}
              <SoundSelect
                id="outcome.sound_id"
                name="outcome.sound_id"
                label="Sound"
                selected={$data.outcome.sound_id}
                onChangeSelected={(selected) =>
                  setFields("outcome.sound_id", selected, true)}
              />
            {/if}
          </FormSection>
        </Tabs.Content>
        <Tabs.Content value="cooldown">
          <!-- Cooldown and role requirements -->
          <FormSection
            title="Delays, cooldown, and requirements"
            description="Configure any delays, cooldown, or requirements on this events trigger"
          >
            <RequiredRoleSelect
              id="require_role"
              name="require_role"
              label="Minimum Required Role"
              selected={$data.require_role}
              onChangeSelected={(selected) =>
                setFields("require_role", selected, true)}
            />

            <FormNumberInput id="cooldown" name="cooldown" label="Cooldown" />
            <FormNumberInput
              id="outcome_delay"
              name="outcome_delay"
              label="Outcome Delay"
            />
          </FormSection>
        </Tabs.Content>
      </Tabs.Root>
    </div>
  </PageLayoutList>
</form>

<style>
  form {
    height: 100%;
  }

  .throwable-config-grid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 1rem;
  }

  .content {
    position: relative;
    flex: auto;
    overflow: hidden;
    height: 100%;
  }

  .content :global([data-tabs-root]) {
    height: 100%;
    display: flex;
    flex-flow: column;
  }

  .content :global([data-tabs-content]) {
    position: relative;
    flex: auto;
    overflow: auto;
    flex-flow: column;
    border: 1px solid #333;
    padding: 1rem;
  }
</style>
