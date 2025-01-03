export type RuntimeAppData = {
  model_id: string | null;
  vtube_studio_connected: boolean;
  vtube_studio_auth: boolean;
  hotkeys: VTubeStudioHotkey[];
  active_overlay_count: number;
};

export type VTubeStudioHotkey = {
  hotkey_id: string;
  name: string;
};
