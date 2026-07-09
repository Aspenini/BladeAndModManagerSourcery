# BladeAndModManagerSourcery

A clean desktop mod manager for **Blade & Sorcery** (PCVR), built with **Rust + Tauri 2** and **React**.

- Detects your game install (Steam / Oculus) and asks you to confirm
- Falls back to a folder picker if the game isn’t found
- Browses and installs mods from **Nexus Mods** (`bladeandsorcery`)
- Installs into `BladeAndSorcery_Data\StreamingAssets\Mods`
- Enable / disable / uninstall local mods; import ZIP archives
- Stores your Nexus API key in **Windows Credential Manager** (never in plain-text config)

## Requirements

- Windows
- [Bun](https://bun.sh/) 1.1+
- [Rust](https://rustup.rs/) (stable)
- Microsoft C++ Build Tools (for compiling Tauri)
- Blade & Sorcery PCVR installed (optional for UI development)
- A [Nexus Mods](https://www.nexusmods.com/) account + personal API key (for browse/download)

> **Note:** Keep the project path free of `&` if possible (e.g. avoid `B&S-...`). Ampersands break some Windows toolchains. This project uses Bun, which handles that path more reliably than npm.

## Develop

```bash
bun install
bun run tauri:dev
```

## Build

```bash
bun run tauri:build
```

## First run

1. Confirm the detected game path (or choose the folder that contains `BladeAndSorcery_Data`).
2. Open **Settings** and paste your Nexus API key from  
   [Account → Site preferences → API](https://www.nexusmods.com/users/myaccount?tab=api).
3. Use **Browse Nexus** to find mods, or **Import ZIP** for files you already downloaded.

## Data locations

| What | Where |
|------|--------|
| Config (`config.json`) | `%AppData%\BladeAndModManagerSourcery\` |
| Installed-mod index | same folder (`installed.json`) |
| Logs | `%AppData%\BladeAndModManagerSourcery\logs\app-YYYY-MM-DD.log` |
| Nexus download cache | `%AppData%\BladeAndModManagerSourcery\downloads\` |
| Nexus API key | Windows Credential Manager (`BladeAndModManagerSourcery` / `nexus-api-key`) |
| Mod folders | `<game>\BladeAndSorcery_Data\StreamingAssets\Mods\` |

## Notes

- **Free vs Premium Nexus**:
  - **Premium**: direct in-app download and install from Browse.
  - **Free**: the app registers as the `nxm://` protocol handler. Use **Download through Nexus** on a file, then on the website choose **Mod Manager Download** → **Slow Download**. The temporary `key` / `expires` / `user_id` arrive via the NXM link and the app queues and installs automatically.
- Not affiliated with WarpFrog or Nexus Mods. Respect mod authors and Nexus Terms of Service.

## Project layout

- `src/` — React UI
- `src-tauri/` — Rust backend (detection, install, Nexus client)
