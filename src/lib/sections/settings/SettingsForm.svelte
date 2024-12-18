<script lang="ts">
  import { z } from "zod";
  import { Tabs } from "bits-ui";
  import { createForm } from "felte";
  import { toast } from "svelte-sonner";
  import reporterDom from "@felte/reporter-dom";
  import { minMax } from "$lib/utils/validation";
  import { validator } from "@felte/validator-zod";
  import { toastErrorMessage } from "$lib/utils/error";
  import PageLayoutList from "$lib/layouts/PageLayoutList.svelte";
  import FormSection from "$lib/components/form/FormSection.svelte";
  import FormSections from "$lib/components/form/FormSections.svelte";
  import SolarBallsBoldDuotone from "~icons/solar/balls-bold-duotone";
  import FormTextInput from "$lib/components/form/FormTextInput.svelte";
  import DetectVTubeStudio from "$lib/components/DetectVTubeStudio.svelte";
  import SolarSettingsBoldDuotone from "~icons/solar/settings-bold-duotone";
  import FormNumberInput from "$lib/components/form/FormNumberInput.svelte";
  import FormBoundCheckbox from "$lib/components/form/FormBoundCheckbox.svelte";
  import SolarShareCircleBoldDuotone from "~icons/solar/share-circle-bold-duotone";
  import SolarPeopleNearbyBoldDuotone from "~icons/solar/people-nearby-bold-duotone";
  import SolarHeadphonesRoundBoldDuotone from "~icons/solar/headphones-round-bold-duotone";
  import {
    type AppData,
    EYES_MODE_VALUES,
    THROW_DIRECTION_VALUES,
  } from "$lib/api/types";
  import {
    getAppData,
    createAppDateMutation,
    createUpdateSettingsMutation,
  } from "$lib/api/runtimeAppData";

  import EyesModeSelect from "./EyesModeSelect.svelte";
  import ThrowableDirectionSelect from "./ThrowableDirectionSelect.svelte";

  const appData = getAppData();
  const appDataMutation = createAppDateMutation();

  const updateSettings = createUpdateSettingsMutation(appData, appDataMutation);

  const schema = z.object({
    // Schema for throwables configuration
    throwables: z.object({
      duration: z.number(),
      spin_speed: minMax,
      throw_angle: minMax,
      direction: z.enum(THROW_DIRECTION_VALUES),
      impact_delay: z.number(),
      item_scale: minMax,
    }),
    // Schema for model related configuration
    model: z.object({
      model_return_time: z.number(),
      eyes_on_hit: z.enum(EYES_MODE_VALUES),
    }),
    // Schema for sound configuration
    sounds: z.object({
      global_volume: z.number(),
    }),
    // Schema for vtube studio configuration
    vtube_studio: z.object({
      host: z.string(),
      port: z.number(),
    }),

    // Schema for external configuration
    external: z.object({
      tts_monster_api_key: z.string(),
    }),

    main: z.object({
      minimize_to_tray: z.boolean(),
      clean_logs: z.boolean(),
      clean_logs_days: z.number(),
      clean_executions: z.boolean(),
      clean_executions_days: z.number(),
    }),
  });

  type Schema = z.infer<typeof schema>;

  function createFromExisting(appData: AppData): Schema {
    const {
      throwables_config,
      model_config,
      sounds_config,
      vtube_studio_config,
      externals_config,
      main_config,
    } = appData;

    return {
      throwables: {
        duration: throwables_config.duration,
        spin_speed: throwables_config.spin_speed,
        throw_angle: throwables_config.throw_angle,
        direction: throwables_config.direction,
        impact_delay: throwables_config.impact_delay,
        item_scale: throwables_config.item_scale,
      },
      model: {
        model_return_time: model_config.model_return_time,
        eyes_on_hit: model_config.eyes_on_hit,
      },
      sounds: {
        global_volume: sounds_config.global_volume,
      },
      vtube_studio: {
        host: vtube_studio_config.host,
        port: vtube_studio_config.port,
      },
      external: {
        tts_monster_api_key: externals_config.tts_monster_api_key ?? "",
      },
      main: {
        minimize_to_tray: main_config.minimize_to_tray,
        clean_logs: main_config.clean_logs,
        clean_logs_days: main_config.clean_logs_days,
        clean_executions: main_config.clean_executions,
        clean_executions_days: main_config.clean_executions_days,
      },
    };
  }

  const { form, data, setFields } = createForm<z.infer<typeof schema>>({
    initialValues: createFromExisting($appData),

    // Validation and error reporting
    extend: [validator({ schema }), reporterDom()],

    async onSubmit(values) {
      const savePromise = save(values);

      toast.promise(savePromise, {
        loading: "Saving settings...",
        success: "Saved settings",
        error: toastErrorMessage("Failed to save settings"),
      });

      await savePromise;
    },
  });

  async function save(values: Schema) {
    const { throwables, model, sounds, vtube_studio, external, main } = values;

    await $updateSettings({
      throwables_config: {
        duration: throwables.duration,
        spin_speed: throwables.spin_speed,
        throw_angle: throwables.throw_angle,
        direction: throwables.direction,
        impact_delay: throwables.impact_delay,
        item_scale: throwables.item_scale,
      },
      model_config: {
        model_return_time: model.model_return_time,
        eyes_on_hit: model.eyes_on_hit,
      },
      sounds_config: {
        global_volume: sounds.global_volume,
      },
      vtube_studio_config: {
        host: vtube_studio.host,
        port: vtube_studio.port,
      },
      externals_config: {
        tts_monster_api_key:
          external.tts_monster_api_key.trim().length < 1
            ? null
            : external.tts_monster_api_key,
      },
      main_config: {
        minimize_to_tray: main.minimize_to_tray,
        clean_logs: main.clean_logs,
        clean_logs_days: main.clean_logs_days,
        clean_executions: main.clean_executions,
        clean_executions_days: main.clean_executions_days,
      },
    });
  }
</script>

<form use:form class="container">
  {#snippet actions()}
    <button type="submit" class="btn">Save</button>
  {/snippet}

  <PageLayoutList
    title="Settings"
    description="Configuration for the entire app"
    {actions}
  >
    <div class="container">
      <Tabs.Root>
        <Tabs.List>
          <Tabs.Trigger value="main">
            <SolarSettingsBoldDuotone /> Main
          </Tabs.Trigger>
          <Tabs.Trigger value="throwables">
            <SolarBallsBoldDuotone /> Throwables
          </Tabs.Trigger>
          <Tabs.Trigger value="sounds">
            <SolarHeadphonesRoundBoldDuotone /> Sounds
          </Tabs.Trigger>
          <Tabs.Trigger value="vtube_studio">
            <SolarSettingsBoldDuotone /> VTube Studio
          </Tabs.Trigger>
          <Tabs.Trigger value="model">
            <SolarPeopleNearbyBoldDuotone /> VTuber Model
          </Tabs.Trigger>
          <Tabs.Trigger value="external">
            <SolarShareCircleBoldDuotone /> External APIs
          </Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="main">
          <FormSections>
            <FormSection title="App">
              <p class="helper">
                Enabling "Minimize to tray" allows you to close the app when
                you're not managing your throwables while streaming to greatly
                reduce its resource usage. When minimized it can be re-opened or
                quit fully from the tray icon.
                <br />
                <br />
                Turn off this setting if you want the app to close fully when close
                is pushed.
              </p>

              <FormBoundCheckbox
                id="main.minimize_to_tray"
                name="main.minimize_to_tray"
                label="Minimize to tray"
                description="Enable minimizing to tray on close instead of closing the app"
              />
            </FormSection>
            <FormSection
              title="Logging"
              description="VTFTK keeps track of logging messages when running scripts and commands, you can automatically clear them after time has passed in order to save space"
            >
              <p class="helper">
                You can view and delete logs for individual scripts manually
                using the "Logs" tab when editing the script/command
              </p>

              <FormBoundCheckbox
                id="main.clean_logs"
                name="main.clean_logs"
                label="Automatically clean logs"
                description="Disable this to prevent automatic log clearing"
              />

              <FormNumberInput
                id="main.clean_logs_days"
                name="main.clean_logs_days"
                label="Retain days"
                description="Number of days logs will be retained for"
                min={0}
              />
            </FormSection>
            <FormSection
              title="Executions"
              description="VTFTK keeps tracks executions of commands and events, this allows it to keep track of cooldown and show you who's triggered a command or event"
            >
              <FormBoundCheckbox
                id="main.clean_executions"
                name="main.clean_executions"
                label="Automatically clean executions"
                description="Disable this to prevent automatic log clearing"
              />

              <FormNumberInput
                id="main.clean_executions_days"
                name="main.clean_executions_days"
                label="Retain days"
                description="Number of days executions will be retained for"
                min={0}
              />
            </FormSection>
          </FormSections>
        </Tabs.Content>
        <Tabs.Content value="throwables">
          <FormSections>
            <FormSection title="Duration and delay">
              <FormNumberInput
                id="throwables.duration"
                name="throwables.duration"
                label="Duration"
                description=" Total time that it should take for a thrown item to hit the target"
              />

              <FormNumberInput
                id="throwables.impact_delay"
                name="throwables.impact_delay"
                label="Impact Delay"
                description="Delay before the impact is registered"
              />
            </FormSection>

            <!-- Spin speed -->
            <FormSection title="Spin speed">
              <div class="row">
                <FormNumberInput
                  id="throwables.spin_speed.min"
                  name="throwables.spin_speed.min"
                  label="Minimum Spin Speed"
                  description="Minimum speed an item can spin at"
                />

                <FormNumberInput
                  id="throwables.spin_speed.max"
                  name="throwables.spin_speed.max"
                  label="Maximum Spin Speed"
                  description="Maximum speed an item can spin at"
                />
              </div>
            </FormSection>

            <FormSection title="Angle and direction">
              <ThrowableDirectionSelect
                id="throwables.direction"
                name="throwables.direction"
                label="Direction"
                description="Which directions the items should come from"
                selected={$data.throwables.direction}
                onChangeSelected={(selected) => {
                  setFields("throwables.direction", selected);
                }}
              />

              <!-- Throw angle -->
              <div class="row">
                <FormNumberInput
                  id="throwables.throw_angle.min"
                  name="throwables.throw_angle.min"
                  label="Minimum Throw Angle"
                  description="Minimum angle an item will be throw at"
                />

                <FormNumberInput
                  id="throwables.throw_angle.max"
                  name="throwables.throw_angle.max"
                  label="Maximum Throw Angle"
                  description="Maximum angle an item will be throw at"
                />
              </div>
            </FormSection>

            <FormSection title="Scale">
              <!-- Item scale -->
              <div class="row">
                <FormNumberInput
                  id="throwables.item_scale.min"
                  name="throwables.item_scale.min"
                  label="Minimum Scale"
                  description="Minimum scale applied to an item"
                />

                <FormNumberInput
                  id="throwables.item_scale.max"
                  name="throwables.item_scale.max"
                  label="Maximum Scale"
                  description="Maximum scale applied to an item"
                />
              </div>
            </FormSection>
          </FormSections>
        </Tabs.Content>
        <Tabs.Content value="sounds">
          <FormSections>
            <FormSection title="Volume">
              <FormNumberInput
                id="sounds.global_volume"
                name="sounds.global_volume"
                label="Global Volume"
                description="Overall volume of all sounds, including impact sounds"
              />

              <!-- TODO: Sound alerts volume, impact sound volume -->
            </FormSection>
          </FormSections>
        </Tabs.Content>
        <Tabs.Content value="vtube_studio">
          <FormSections>
            <FormSection
              title="API Settings"
              description="Details for the VTube Studio API"
            >
              <div class="row row-ll">
                <FormTextInput
                  id="vtube_studio.host"
                  name="vtube_studio.host"
                  label="Host"
                  description="Host to use when connecting to VTube Studio"
                />

                <button
                  type="button"
                  class="btn"
                  onclick={() => {
                    setFields("vtube_studio.host", "localhost");
                  }}>Default</button
                >
              </div>

              <FormNumberInput
                id="vtube_studio.port"
                name="vtube_studio.port"
                label="Port"
                description="Port that the VTube Studio API is running on"
              />

              <DetectVTubeStudio
                onChoosePort={(port) => setFields("vtube_studio.port", port)}
              />
            </FormSection>
          </FormSections>
        </Tabs.Content>
        <Tabs.Content value="model">
          <FormSections>
            <FormSection title="Model Settings">
              <FormNumberInput
                id="model.model_return_time"
                name="model.model_return_time"
                label="Return Time"
                description="Time it takes for the model to return to its original position after being hit"
              />

              <EyesModeSelect
                id="model.eyes_on_hit"
                name="model.eyes_on_hit"
                label="Eyes On Hit"
                description="How the model eyes should react to being hit"
                selected={$data.model.eyes_on_hit}
                onChangeSelected={(selected) => {
                  setFields("model.eyes_on_hit", selected);
                }}
              />
            </FormSection>
          </FormSections>
        </Tabs.Content>
        <Tabs.Content value="external">
          <FormSections>
            <FormSection title="TTS Monster API Key">
              <FormTextInput
                id="external.tts_monster_api_key"
                name="external.tts_monster_api_key"
                label="TTS Monster API Key"
                description="API Key to use TTS monster TTS voice generation"
                type="password"
              />
            </FormSection>
          </FormSections>
        </Tabs.Content>
      </Tabs.Root>
    </div>
  </PageLayoutList>
</form>

<style>
  .helper {
    font-size: 0.8rem;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    align-items: center;
    justify-content: center;
  }

  .row-ll {
    grid-template-columns: 3fr 1fr;
  }

  .container {
    position: relative;
    flex: auto;
    overflow: hidden;

    display: flex;
    flex-flow: column;
    gap: 0.5rem;

    height: 100%;
  }

  .container :global([data-tabs-root]) {
    height: 100%;
    display: flex;
    flex-flow: column;
  }

  .container :global([data-tabs-content]) {
    position: relative;
    flex: auto;
    overflow: auto;
    flex-flow: column;
    border: 1px solid #333;
    padding: 1rem;
  }
</style>
