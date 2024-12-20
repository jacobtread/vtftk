<script lang="ts">
  import { z } from "zod";
  import { Tabs } from "bits-ui";
  import { createForm } from "felte";
  import { toast } from "svelte-sonner";
  import { goto } from "$app/navigation";
  import reporterDom from "@felte/reporter-dom";
  import { validator } from "@felte/validator-zod";
  import { toastErrorMessage } from "$lib/utils/error";
  import PageLayoutList from "$lib/layouts/PageLayoutList.svelte";
  import { createCommand, updateCommand } from "$lib/api/commands";
  import FormSection from "$lib/components/form/FormSection.svelte";
  import CodeEditor from "$lib/components/scripts/CodeEditor.svelte";
  import FormSections from "$lib/components/form/FormSections.svelte";
  import FormTextInput from "$lib/components/form/FormTextInput.svelte";
  import SolarReorderBoldDuotone from "~icons/solar/reorder-bold-duotone";
  import FormNumberInput from "$lib/components/form/FormNumberInput.svelte";
  import SolarSettingsBoldDuotone from "~icons/solar/settings-bold-duotone";
  import FormBoundCheckbox from "$lib/components/form/FormBoundCheckbox.svelte";
  import SolarCodeSquareBoldDuotone from "~icons/solar/code-square-bold-duotone";
  import {
    type Command,
    CommandOutcomeType,
    MinimumRequiredRole,
    MINIMUM_REQUIRED_ROLE_VALUES,
  } from "$lib/api/types";

  import CommandLogs from "./CommandLogs.svelte";
  import CommandExecutions from "./CommandExecutions.svelte";
  import CommandOutcomeSelect from "./CommandOutcomeSelect.svelte";
  import exampleCode from "../../../../script/example_command.js?raw";
  import RequiredRoleSelect from "../events/RequiredRoleSelect.svelte";

  type Props = {
    existing?: Command;
  };

  const { existing }: Props = $props();

  const outcomeSchema = z.discriminatedUnion("type", [
    z.object({
      type: z.literal(CommandOutcomeType.Template),
      message: z.string(),
    }),
    z.object({
      type: z.literal(CommandOutcomeType.Script),
      script: z.string(),
    }),
  ]);

  type OutcomeSchema = z.infer<typeof outcomeSchema>;

  const schema = z.object({
    name: z.string().min(1, "You must specify a name"),
    command: z.string().min(1, "You must specify a command"),
    enabled: z.boolean(),
    outcome: outcomeSchema,
    require_role: z.enum(MINIMUM_REQUIRED_ROLE_VALUES),
    cooldown: z.number(),
  });

  type Schema = z.infer<typeof schema>;

  const createDefaults: Schema = {
    name: "",
    command: "!test",
    enabled: true,
    outcome: getOutcomeDefaults(CommandOutcomeType.Script),
    require_role: MinimumRequiredRole.None,
    cooldown: 1000,
  };

  function createFromExisting(config: Command): Partial<Schema> {
    return {
      name: config.name,
      command: config.command,
      enabled: config.enabled,
      outcome: config.outcome,
      require_role: config.require_role,
      cooldown: config.cooldown,
    };
  }

  const { form, data, setFields, isDirty, setIsDirty } = createForm<
    z.infer<typeof schema>
  >({
    // Derive initial values
    initialValues: existing ? createFromExisting(existing) : createDefaults,

    // Validation and error reporting
    extend: [validator({ schema }), reporterDom()],

    onSubmit(values) {
      saveWithToast(values);

      if (!existing) {
        goto("/commands");
      }
    },
  });

  function saveWithToast(values: Schema) {
    const savePromise = save(values);

    toast.promise(
      savePromise,
      existing
        ? {
            loading: "Saving command...",
            success: "Saved command",
            error: toastErrorMessage("Failed to save command"),
          }
        : {
            loading: "Creating command...",
            success: "Created command",
            error: toastErrorMessage("Failed to create command"),
          },
    );

    return savePromise;
  }

  async function save(values: Schema) {
    const command = values.command.toLowerCase().trim();

    if (existing !== undefined) {
      await updateCommand({
        commandId: existing.id,
        update: {
          enabled: values.enabled,
          name: values.name,
          command,
          aliases: [],
          outcome: values.outcome,
          cooldown: values.cooldown,
          require_role: values.require_role,
        },
      });
    } else {
      await createCommand({
        enabled: values.enabled,
        name: values.name,
        command,
        aliases: [],
        outcome: values.outcome,
        cooldown: values.cooldown,
        require_role: values.require_role,
      });
    }

    setIsDirty(false);
  }

  function getOutcomeDefaults(type: CommandOutcomeType): OutcomeSchema {
    switch (type) {
      case CommandOutcomeType.Template:
        return {
          type: CommandOutcomeType.Template,
          message: "",
        };

      case CommandOutcomeType.Script:
        return {
          type: CommandOutcomeType.Script,
          script: exampleCode,
        };
    }
  }

  function onChangeOutcomeType(type: CommandOutcomeType) {
    const defaults = getOutcomeDefaults(type);
    setFields("outcome", defaults, true);
  }
</script>

<form use:form>
  {#snippet actions()}
    <button type="submit" class="btn">
      {existing ? "Save" : "Create"}
    </button>
    <a class="btn" href="/commands">Back</a>
  {/snippet}

  <PageLayoutList
    title={existing ? "Edit Command" : "Create Command"}
    description={existing && $isDirty ? "Unsaved changes..." : "..."}
    {actions}
  >
    <div class="content">
      <Tabs.Root let:value>
        <Tabs.List>
          <Tabs.Trigger value="settings">
            <SolarSettingsBoldDuotone /> Settings
          </Tabs.Trigger>
          <Tabs.Trigger value="code">
            <SolarCodeSquareBoldDuotone /> Code
          </Tabs.Trigger>
          {#if existing !== undefined}
            <Tabs.Trigger value="executions">
              <SolarReorderBoldDuotone /> Executions
            </Tabs.Trigger>
            <Tabs.Trigger value="logs">
              <SolarReorderBoldDuotone /> Logs
            </Tabs.Trigger>
          {/if}
        </Tabs.List>
        <Tabs.Content value="code">
          {#if $data.outcome.type === CommandOutcomeType.Script}
            <section class="editor">
              <CodeEditor
                value={$data.outcome.script}
                onChange={(value) => {
                  setFields("outcome.script", value, true);
                  setIsDirty(true);
                }}
                onUserSave={() => {
                  if (existing) saveWithToast($data);
                }}
              />
            </section>
          {:else if $data.outcome.type === CommandOutcomeType.Template}
            <textarea
              id="outcome.script"
              name="outcome.script"
              style="width: 100%;height:100%"
            ></textarea>
          {/if}
        </Tabs.Content>
        <Tabs.Content value="settings">
          <FormSections>
            <FormSection
              title="Details"
              description="Basic details about the command"
            >
              <div class="row">
                <FormTextInput
                  id="name"
                  name="name"
                  label="Name"
                  description="Name for the command"
                />
                <FormTextInput
                  id="command"
                  name="command"
                  label="Command"
                  description="Message that will trigger this command"
                />
              </div>

              <FormBoundCheckbox
                id="enabled"
                name="enabled"
                label="Enabled"
                description="Whether this command can be used"
              />

              <CommandOutcomeSelect
                id="outcome.type"
                name="outcome.type"
                label="Command Type"
                selected={$data.outcome.type}
                onChangeSelected={(selected) => {
                  onChangeOutcomeType(selected);
                }}
              />
            </FormSection>

            <!-- Cooldown and role requirements -->
            <FormSection
              title="Cooldown, and requirements"
              description="Configure any cooldown, or requirements on this command trigger"
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
            </FormSection>
          </FormSections>
        </Tabs.Content>
        {#if existing !== undefined}
          <Tabs.Content value="logs">
            {#if value === "logs"}
              <CommandLogs id={existing.id} />
            {/if}
          </Tabs.Content>
          <Tabs.Content value="executions">
            {#if value === "executions"}
              <CommandExecutions id={existing.id} />
            {/if}
          </Tabs.Content>
        {/if}
      </Tabs.Root>
    </div>
  </PageLayoutList>
</form>

<style>
  .editor {
    position: relative;
    overflow: hidden;
    height: 100%;
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
  }

  .content :global([data-tabs-content]:nth-child(3)) {
    padding: 1rem;
  }

  form {
    height: 100%;
    display: flex;
    flex-flow: column;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }
</style>
