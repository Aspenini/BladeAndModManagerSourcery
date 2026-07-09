import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type Locale = "old" | "modern";

const STORAGE_KEY = "bams-locale";

const modern = {
  // Nav / chrome
  navLibrary: "Library",
  navBrowse: "Browse Nexus",
  navSettings: "Settings",
  topbarLibrary: "Installed mods",
  topbarBrowse: "Nexus Mods · bladeandsorcery",
  topbarSettings: "Preferences",
  nexusConnected: "Nexus connected",
  nexusPremium: " · Premium",
  nexusFree: " · Free",
  nexusKeyMissing: "Nexus key missing",
  starting: "Starting BladeAndModManagerSourcery…",
  retry: "Retry",

  // Library
  libraryTitle: "Library",
  libraryDesc:
    "Mods installed in your Blade & Sorcery StreamingAssets\\Mods folder.",
  browseNexus: "Browse Nexus",
  importZip: "Import ZIP",
  refresh: "Refresh",
  loadingMods: "Loading installed mods…",
  noModsTitle: "No mods installed yet",
  noModsBody: "Browse Nexus Mods or import a ZIP you already downloaded.",
  filterMods: "Filter installed mods…",
  filterAll: "All ({n})",
  filterEnabled: "Enabled ({n})",
  filterDisabled: "Disabled ({n})",
  noMatchTitle: "No matching mods",
  noMatchBody: "Try clearing the filter or changing the status selection.",
  enabled: "Enabled",
  disabled: "Disabled",
  noManifest: "no manifest",
  open: "Open",
  uninstall: "Uninstall",
  confirmUninstall:
    "Uninstall “{name}”? This deletes the mod folder.",
  removed: "Removed {name}",
  installed: "Installed “{name}”",
  importTitle: "Import mod ZIP",
  modArchive: "Mod archive",

  // Browse
  browseTitle: "Browse Nexus",
  browseDesc: "Discover Blade & Sorcery mods and install the ones you want to try.",
  apiKeyRequiredTitle: "API key required",
  apiKeyRequiredBody:
    "Create a key at nexusmods.com (Profile → Site preferences → API), then paste it in Settings.",
  openSettings: "Open Settings",
  connectApiDesc:
    "Connect your Nexus Mods account with a personal API key to search and download mods.",
  backToResults: "← Back to results",
  waitingNexus:
    "Waiting for Nexus… choose Mod Manager Download → Slow Download.",
  loadingMod: "Loading mod…",
  couldNotLoadMod: "Couldn't load this mod",
  checkConnection: "Check your connection, then try again.",
  tryAgain: "Try again",
  noImage: "No image",
  byAuthor: "by {name}",
  unknownAuthor: "Unknown author",
  openOnNexus: "Open on Nexus",
  freeAccountHint:
    "Free Nexus accounts can't use the direct download API. Use Download through Nexus, then on the website choose Mod Manager Download → Slow Download. This app is registered for nxm:// links and will queue the install automatically.",
  files: "Files",
  noFiles: "No files listed for this mod.",
  mainFile: "main",
  install: "Install",
  downloadThroughNexus: "Download through Nexus",
  downloadThroughNexusTitle:
    "Opens Nexus so you can use Mod Manager Download",
  searchMods: "Search mods…",
  search: "Search",
  sortTrending: "Trending",
  sortLatest: "Latest added",
  sortUpdated: "Recently updated",
  loadingFromNexus: "Loading mods from Nexus…",
  noModsFound: "No mods found",
  tryAnotherSearch: "Try another search or sort option.",
  unknown: "Unknown",
  waitingNexusToast:
    "Waiting for Nexus… Mod Manager Download → Slow Download",
  downloadingMod: "Downloading mod {modId} file {fileId}…",
  modInstalled: "Mod installed",

  // Settings
  settingsTitle: "Settings",
  settingsDesc: "Game location and Nexus Mods connection.",
  gameSection: "Game",
  installPath: "Install path",
  notSet: "Not set",
  changeFolder: "Change folder…",
  openModsFolder: "Open Mods folder",
  modsDirectory: "Mods directory",
  nexusKeySection: "Nexus Mods API key",
  nexusKeyHelp:
    "Your personal key is stored in the Windows Credential Manager (not in config files). Premium accounts download directly in the app; free accounts use Download through Nexus (Mod Manager Download → Slow Download) via the nxm:// protocol this app registers. Get a key here:",
  nexusApiLink: "nexusmods.com → API",
  pasteKey: "Paste API key",
  replaceKey: "Enter a new key to replace the stored one",
  saveValidate: "Save & validate",
  removeKey: "Remove key",
  signedInAs: "Signed in as {name}",
  keyStored: "API key stored securely",
  freeNxm: " · Free (NXM downloads)",
  aboutSection: "About",
  aboutBody:
    "BladeAndModManagerSourcery for Blade & Sorcery (PCVR). Not affiliated with WarpFrog or Nexus Mods. Use at your own risk; back up saves and respect mod authors' licenses.",
  runSetupAgain: "Run game setup again…",
  confirmSetupAgain: "Run setup again? Your API key is kept.",
  languageSection: "Language",
  languageDesc:
    "Speak in the tongue of olden days, or plain modern English.",
  oldEnglish: "Old English",
  oldEnglishHint: "Archaic speech for the scriptorium (default).",
  connectedPremium: "Connected as {name} (Premium)",
  connectedFree: "Connected as {name} (free)",
  apiKeyRemoved: "API key removed",
  gamePathUpdated: "Game path updated",
  selectGameFolder: "Select Blade & Sorcery game folder",
  nexusUser: "Nexus user",
  user: "user",

  // Setup
  findGame: "Find your game",
  findGameDesc:
    "We look for a Steam or Oculus install of Blade & Sorcery. Confirm the path, or choose the folder yourself.",
  scanning: "Scanning for Blade & Sorcery…",
  isThisInstall: "Is this your install?",
  steam: "Steam",
  oculus: "Oculus",
  manual: "Manual",
  couldNotFind:
    "Couldn't find Blade & Sorcery automatically. Browse to the game folder that contains BladeAndSorcery_Data.",
  yesUseFolder: "Yes, use this folder",
  chooseDifferent: "Choose different folder…",
  typicalPath: "Typical Steam path:",
  modsInstallTo: "Mods install to",
  selectedPath: "Selected path",
} as const;

export type MessageKey = keyof typeof modern;

const old: Record<MessageKey, string> = {
  navLibrary: "The Library",
  navBrowse: "Seek the Nexus",
  navSettings: "The Scriptorium",
  topbarLibrary: "The library of installed enchantments",
  topbarBrowse: "Scrolls from the Nexus · bladeandsorcery",
  topbarSettings: "Scriptorium preferences",
  nexusConnected: "Bound to the Nexus",
  nexusPremium: " · Prestige",
  nexusFree: " · Commoner",
  nexusKeyMissing: "The key is wanting",
  starting: "Awakening the grimoire…",
  retry: "Attempt again",

  libraryTitle: "The Library",
  libraryDesc:
    "Enchantments laid within thy Blade & Sorcery StreamingAssets\\Mods vault.",
  browseNexus: "Seek the Nexus",
  importZip: "Bring forth a ZIP",
  refresh: "Renew",
  loadingMods: "Fetching the rolls of enchantments…",
  noModsTitle: "No enchantments yet dwell herein",
  noModsBody:
    "Seek the Nexus, or bring forth a ZIP thou hast already claimed.",
  filterMods: "Sift the enchantments…",
  filterAll: "All ({n})",
  filterEnabled: "Awake ({n})",
  filterDisabled: "Slumbering ({n})",
  noMatchTitle: "None match thy query",
  noMatchBody: "Clear the filter, or choose another status of the rolls.",
  enabled: "Awake",
  disabled: "Slumbering",
  noManifest: "no charter",
  open: "Reveal",
  uninstall: "Banish",
  confirmUninstall:
    "Banish “{name}”? This shall smite the enchantment's folder.",
  removed: "Banished {name}",
  installed: "Inscribed “{name}”",
  importTitle: "Bring forth a mod ZIP",
  modArchive: "Mod archive",

  browseTitle: "Seek the Nexus",
  browseDesc:
    "Discover enchantments of Blade & Sorcery, and inscribe those thou wouldst wield.",
  apiKeyRequiredTitle: "A key is required",
  apiKeyRequiredBody:
    "Fashion a key at nexusmods.com (Profile → Site preferences → API), then paste it within the Scriptorium.",
  openSettings: "Open the Scriptorium",
  connectApiDesc:
    "Bind thy Nexus Mods account with a personal key, that thou mayest search and claim enchantments.",
  backToResults: "← Return to the rolls",
  waitingNexus:
    "Awaiting the Nexus… choose Mod Manager Download → Slow Download.",
  loadingMod: "Unfurling the scroll…",
  couldNotLoadMod: "This scroll would not open",
  checkConnection: "Mind thy connection, then attempt again.",
  tryAgain: "Attempt again",
  noImage: "No likeness",
  byAuthor: "by the hand of {name}",
  unknownAuthor: "Author unknown",
  openOnNexus: "Open upon the Nexus",
  freeAccountHint:
    "Commoner accounts of the Nexus cannot seize files by direct art. Use Download through Nexus, then upon the website choose Mod Manager Download → Slow Download. This grimoire is registered for nxm:// links and shall queue the inscription of its own accord.",
  files: "Tomes & fragments",
  noFiles: "No fragments are listed for this enchantment.",
  mainFile: "chief",
  install: "Inscribe",
  downloadThroughNexus: "Claim via the Nexus",
  downloadThroughNexusTitle:
    "Opens the Nexus so thou mayest use Mod Manager Download",
  searchMods: "Search the rolls…",
  search: "Seek",
  sortTrending: "Of renown",
  sortLatest: "Newly scribed",
  sortUpdated: "Of late renewed",
  loadingFromNexus: "Drawing scrolls from the Nexus…",
  noModsFound: "No enchantments found",
  tryAnotherSearch: "Try another query or ordering of the rolls.",
  unknown: "Unknown",
  waitingNexusToast:
    "Awaiting the Nexus… Mod Manager Download → Slow Download",
  downloadingMod: "Fetching enchantment {modId}, fragment {fileId}…",
  modInstalled: "Enchantment inscribed",

  settingsTitle: "The Scriptorium",
  settingsDesc: "The game's dwelling-place and thy bond to the Nexus.",
  gameSection: "The Game",
  installPath: "Path of the install",
  notSet: "Not yet set",
  changeFolder: "Choose another vault…",
  openModsFolder: "Open the Mods vault",
  modsDirectory: "Directory of Mods",
  nexusKeySection: "Key of the Nexus Mods",
  nexusKeyHelp:
    "Thy personal key is kept within the Windows Credential Manager (not amidst plain config scrolls). Those of Prestige download by direct art; commoners use Claim via the Nexus (Mod Manager Download → Slow Download) by the nxm:// rite this grimoire registers. Obtain a key hither:",
  nexusApiLink: "nexusmods.com → API",
  pasteKey: "Paste thy key",
  replaceKey: "Enter a new key to supplant the stored one",
  saveValidate: "Seal & prove",
  removeKey: "Cast out the key",
  signedInAs: "Known as {name}",
  keyStored: "Key kept in safe keeping",
  freeNxm: " · Commoner (NXM claims)",
  aboutSection: "Of this work",
  aboutBody:
    "BladeAndModManagerSourcery for Blade & Sorcery (PCVR). Not affiliated with WarpFrog or Nexus Mods. Use at thine own peril; keep copies of thy saves, and honour the licenses of mod authors.",
  runSetupAgain: "Run the founding rite again…",
  confirmSetupAgain: "Run setup again? Thy key shall be kept.",
  languageSection: "Tongue",
  languageDesc:
    "Speak in the manner of olden days, or in plain modern English.",
  oldEnglish: "Old English",
  oldEnglishHint: "Archaic speech for the scriptorium.",
  connectedPremium: "Bound as {name} (Prestige)",
  connectedFree: "Bound as {name} (commoner)",
  apiKeyRemoved: "The key is cast out",
  gamePathUpdated: "The game path is renewed",
  selectGameFolder: "Select Blade & Sorcery game folder",
  nexusUser: "Nexus wanderer",
  user: "wanderer",

  findGame: "Find thy game",
  findGameDesc:
    "We seek a Steam or Oculus install of Blade & Sorcery. Confirm the path, or choose the vault thyself.",
  scanning: "Scrying for Blade & Sorcery…",
  isThisInstall: "Is this thy install?",
  steam: "Steam",
  oculus: "Oculus",
  manual: "By hand",
  couldNotFind:
    "Could not find Blade & Sorcery of its own accord. Browse to the vault that contains BladeAndSorcery_Data.",
  yesUseFolder: "Aye, use this vault",
  chooseDifferent: "Choose another vault…",
  typicalPath: "A common Steam path:",
  modsInstallTo: "Enchantments are laid in",
  selectedPath: "Path chosen by hand",
};

const dictionaries: Record<Locale, Record<MessageKey, string>> = {
  modern,
  old,
};

function loadLocale(): Locale {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === "modern" || raw === "old") return raw;
  } catch {
    /* ignore */
  }
  return "old";
}

function format(
  template: string,
  vars?: Record<string, string | number>,
): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_, key: string) =>
    vars[key] != null ? String(vars[key]) : `{${key}}`,
  );
}

type I18nApi = {
  locale: Locale;
  oldEnglish: boolean;
  setLocale: (locale: Locale) => void;
  setOldEnglish: (on: boolean) => void;
  t: (key: MessageKey, vars?: Record<string, string | number>) => string;
};

const I18nContext = createContext<I18nApi | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => loadLocale());

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      /* ignore */
    }
  }, []);

  const setOldEnglish = useCallback(
    (on: boolean) => {
      setLocale(on ? "old" : "modern");
    },
    [setLocale],
  );

  const t = useCallback(
    (key: MessageKey, vars?: Record<string, string | number>) => {
      const dict = dictionaries[locale] ?? dictionaries.old;
      const template = dict[key] ?? modern[key] ?? key;
      return format(template, vars);
    },
    [locale],
  );

  const value = useMemo<I18nApi>(
    () => ({
      locale,
      oldEnglish: locale === "old",
      setLocale,
      setOldEnglish,
      t,
    }),
    [locale, setLocale, setOldEnglish, t],
  );

  return (
    <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
  );
}

export function useI18n(): I18nApi {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return ctx;
}
