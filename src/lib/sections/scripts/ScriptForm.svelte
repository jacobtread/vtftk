<script lang="ts">
  import type { Script } from "$lib/api/types";

  import { z } from "zod";
  import { Tabs } from "bits-ui";
  import { createForm } from "felte";
  import { toast } from "svelte-sonner";
  import { goto } from "$app/navigation";
  import reporterDom from "@felte/reporter-dom";
  import { invoke } from "@tauri-apps/api/core";
  import { validator } from "@felte/validator-zod";
  import { toastErrorMessage } from "$lib/utils/error";
  import { createScript, updateScript } from "$lib/api/scripts";
  import FormSection from "$lib/components/form/FormSection.svelte";
  import CodeEditor from "$lib/components/scripts/CodeEditor.svelte";
  import FormSections from "$lib/components/form/FormSections.svelte";
  import FormTextInput from "$lib/components/form/FormTextInput.svelte";
  import SolarReorderBoldDuotone from "~icons/solar/reorder-bold-duotone";
  import SolarSettingsBoldDuotone from "~icons/solar/settings-bold-duotone";
  import FormBoundCheckbox from "$lib/components/form/FormBoundCheckbox.svelte";
  import SolarCodeSquareBoldDuotone from "~icons/solar/code-square-bold-duotone";

  import ScriptLogs from "./ScriptLogs.svelte";
  // Example code for the editor
  import exampleCode from "../../../../script/example.js?raw";

  type Props = {
    existing?: Script;
  };

  const { existing }: Props = $props();

  const schema = z.object({
    name: z.string().min(1, "You must specify a name"),
    enabled: z.boolean(),
    script: z.string(),
  });

  type Schema = z.infer<typeof schema>;

  // Defaults when creating a new throwable
  const createDefaults: Partial<Schema> = {
    name: "",
    enabled: true,
    script: exampleCode,
  };

  function createFromExisting(config: Script): Schema {
    return {
      name: config.name,
      enabled: config.enabled,
      script: config.script,
    };
  }

  const { form, data, setFields, isDirty, setIsDirty } = createForm<
    z.infer<typeof schema>
  >({
    // Derive initial values
    initialValues: existing ? createFromExisting(existing) : createDefaults,

    // Validation and error reporting
    extend: [validator({ schema }), reporterDom()],

    async onSubmit(values) {
      const savePromise = save(values);

      toast.promise(
        savePromise,
        existing
          ? {
              loading: "Saving script...",
              success: "Saved script",
              error: toastErrorMessage("Failed to save script"),
            }
          : {
              loading: "Creating script...",
              success: "Created script",
              error: toastErrorMessage("Failed to create script"),
            },
      );

      if (!existing) {
        goto("/scripts");
      }
    },
  });

  async function save(values: Schema) {
    // Determine what events the script handles
    const events = await invoke<string[]>("test_get_script_events", {
      script: values.script,
    });

    if (existing !== undefined) {
      await updateScript({
        scriptId: existing.id,
        update: {
          enabled: values.enabled,
          name: values.name,
          script: values.script,
          events,
        },
      });
    } else {
      await createScript({
        enabled: values.enabled,
        name: values.name,
        script: values.script,
        events,
      });
    }

    // Reset dirty state after saving
    setIsDirty(false);
  }
</script>

<form use:form class="container">
  <div class="title-area">
    <div>
      <h1 class="title">{existing ? "Edit Script" : "Create Script"}</h1>
      <p class="text">
        {#if existing && $isDirty}
          Unsaved changes...
        {/if}
      </p>
    </div>
    <div class="actions">
      <button type="submit" class="btn">
        {existing ? "Save" : "Create"}
      </button>
      <a class="btn" href="/scripts">Back</a>
    </div>
  </div>

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
          <Tabs.Trigger value="logs">
            <SolarReorderBoldDuotone /> Logs
          </Tabs.Trigger>
        {/if}
      </Tabs.List>
      <Tabs.Content value="code">
        <section class="editor">
          <CodeEditor
            value={$data.script}
            onChange={(value) => {
              setFields("script", value, true);
              setIsDirty(true);
            }}
            onUserSave={() => {
              if (existing) save($data);
            }}
          />
        </section>
      </Tabs.Content>
      <Tabs.Content value="settings">
        <FormSections>
          <FormSection
            title="Details"
            description="Basic details about the script"
          >
            <FormTextInput id="name" name="name" label="Name" />

            <FormBoundCheckbox id="enabled" name="enabled" label="Enabled" />
          </FormSection>
        </FormSections>
      </Tabs.Content>
      {#if existing !== undefined}
        <Tabs.Content value="logs">
          {#if value === "logs"}
            <ScriptLogs id={existing.id} />
          {/if}
        </Tabs.Content>
      {/if}
    </Tabs.Root>
  </div>
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

  /* Padded outline for the settings tab */
  .content :global([data-tabs-content]:nth-child(3)) {
    padding: 1rem;
  }

  .container {
    display: flex;
    flex-flow: column;
    gap: 0.5rem;

    padding: 1rem;
    height: 100%;
  }

  .title {
    color: #fff;
    margin-bottom: 0.25rem;
    line-height: 1;
    font-size: 1.75rem;
  }

  .text {
    color: #ccc;
  }

  .title-area {
    display: flex;
    align-items: center;
  }

  .actions {
    display: flex;
    flex: auto;
    justify-content: flex-end;
    gap: 1rem;
    align-items: center;
  }
</style>
