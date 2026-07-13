import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AppConfig = {
  gamePath?: string | null;
  setupComplete: boolean;
  /** True when a key is stored in the OS credential manager (never returned as plaintext). */
  hasNexusApiKey?: boolean;
};

export type DetectedGame = {
  path: string;
  source: string;
  valid: boolean;
  message: string;
};

export type GamePaths = {
  gameRoot: string;
  dataDir: string;
  streamingAssets: string;
  modsDir: string;
  exePath?: string | null;
};

export type LocalMod = {
  folderName: string;
  displayName: string;
  author?: string | null;
  version?: string | null;
  description?: string | null;
  /** Game version the mod's manifest declares compatibility with. */
  gameVersion?: string | null;
  enabled: boolean;
  path: string;
  /** False when the folder has no manifest at all — the game ignores it and it can't be toggled. */
  hasManifest: boolean;
  nexusModId?: number | null;
  nexusFileId?: number | null;
  /** Box (local collection) this mod belongs to, if any. */
  boxId?: string | null;
};

export type ModBox = {
  id: string;
  name: string;
  createdAt?: string | null;
};

export type BoxesState = {
  boxes: ModBox[];
  activeBoxId?: string | null;
  /** Mod folder name → box id. */
  assignments: Record<string, string>;
};

export type ActivateReport = {
  boxId: string;
  boxName: string;
  enabled: number;
  disabled: number;
  skipped: string[];
  errors: string[];
};

export type NexusUser = {
  name?: string | null;
  isPremium: boolean;
  isSupporter: boolean;
  profileUrl?: string | null;
};

export type NexusModSummary = {
  modId: number;
  name: string;
  summary?: string | null;
  pictureUrl?: string | null;
  author?: string | null;
  version?: string | null;
  endorsements?: number | null;
  downloads?: number | null;
  categoryId?: number | null;
  createdTime?: string | null;
  updatedTime?: string | null;
  available: boolean;
};

export type NexusModDetail = NexusModSummary & {
  description?: string | null;
  containsAdultContent?: boolean | null;
};

export type NexusFile = {
  fileId: number;
  name: string;
  version?: string | null;
  categoryName?: string | null;
  sizeKb?: number | null;
  uploadedTime?: string | null;
  description?: string | null;
  isPrimary: boolean;
};

export type InstallResult = {
  folderName: string;
  message: string;
  modId?: number | null;
  fileId?: number | null;
};

/** Backend progress events for free-user NXM downloads. */
export type NxmProgressEvent = {
  stage: "started" | "finished" | "error" | string;
  modId: number;
  fileId: number;
  message: string;
  folderName?: string | null;
};

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  saveAppConfig: (config: AppConfig) =>
    invoke<AppConfig>("save_app_config", { config }),
  detectGame: () => invoke<DetectedGame | null>("detect_game_install"),
  validatePath: (path: string) =>
    invoke<DetectedGame>("validate_and_inspect_path", { path }),
  confirmGamePath: (path: string) =>
    invoke<AppConfig>("confirm_game_path", { path }),
  getGamePaths: () => invoke<GamePaths>("get_game_paths"),
  listMods: () => invoke<LocalMod[]>("list_mods"),
  toggleMod: (folderName: string, enabled: boolean) =>
    invoke<string>("toggle_mod", { folderName, enabled }),
  removeMod: (folderName: string) =>
    invoke<void>("remove_mod", { folderName }),
  importArchive: (archivePath: string) =>
    invoke<string>("import_mod_archive", { archivePath }),
  listBoxes: () => invoke<BoxesState>("list_boxes"),
  createBox: (name: string) => invoke<BoxesState>("create_box", { name }),
  renameBox: (boxId: string, name: string) =>
    invoke<BoxesState>("rename_box", { boxId, name }),
  deleteBox: (boxId: string) => invoke<BoxesState>("delete_box", { boxId }),
  assignModBox: (folderName: string, boxId: string | null) =>
    invoke<BoxesState>("assign_mod_box", { folderName, boxId }),
  activateBox: (boxId: string) =>
    invoke<ActivateReport>("activate_box", { boxId }),
  clearActiveBox: () => invoke<BoxesState>("clear_active_box"),
  nexusValidate: (apiKey: string) =>
    invoke<NexusUser>("nexus_validate", { apiKey }),
  nexusSaveApiKey: (apiKey: string) =>
    invoke<NexusUser>("nexus_save_api_key", { apiKey }),
  nexusClearApiKey: () => invoke<void>("nexus_clear_api_key"),
  nexusGetUser: () => invoke<NexusUser>("nexus_get_user"),
  nexusListMods: (sort: string, query?: string) =>
    invoke<NexusModSummary[]>("nexus_list_mods", { sort, query: query ?? null }),
  nexusModDetail: (modId: number) =>
    invoke<NexusModDetail>("nexus_mod_detail", { modId }),
  nexusModFiles: (modId: number) =>
    invoke<NexusFile[]>("nexus_mod_files", { modId }),
  nexusDownloadAndInstall: (
    modId: number,
    fileId: number,
    modName?: string,
    fileVersion?: string,
  ) =>
    invoke<InstallResult>("nexus_download_and_install", {
      modId,
      fileId,
      modName: modName ?? null,
      fileVersion: fileVersion ?? null,
    }),
  nexusDownloadWithNxm: (
    nxmUrl: string,
    modName?: string,
    fileVersion?: string,
  ) =>
    invoke<InstallResult>("nexus_download_with_nxm", {
      nxmUrl,
      modName: modName ?? null,
      fileVersion: fileVersion ?? null,
    }),
  nexusModUrl: (modId: number) => invoke<string>("nexus_mod_url", { modId }),
  nexusFileUrl: (modId: number, fileId: number) =>
    invoke<string>("nexus_file_url", { modId, fileId }),
  getAppInfo: () =>
    invoke<{
      name: string;
      version: string;
      game: string;
      nexusDomain: string;
      steamAppId: string;
    }>("get_app_info"),
};

/** Subscribe to free-user NXM download/install progress from the Rust backend. */
export function onNxmDownload(
  handler: (event: NxmProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<NxmProgressEvent>("nxm-download", (e) => handler(e.payload));
}

export function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
