import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { api, AppConfig, errMessage, NexusUser } from "../lib/tauri";
import { useI18n } from "../lib/i18n";
import { useToast } from "../lib/toast";

type Props = {
  config: AppConfig;
  nexusUser: NexusUser | null;
  onConfigChange: (c: AppConfig) => void;
  onNexusUserChange: (u: NexusUser | null) => void;
  onResetSetup: () => void;
};

export default function Settings({
  config,
  nexusUser,
  onConfigChange,
  onNexusUserChange,
  onResetSetup,
}: Props) {
  const { t, oldEnglish, setOldEnglish } = useI18n();
  const toast = useToast();
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [modsDir, setModsDir] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const paths = await api.getGamePaths();
        setModsDir(paths.modsDir);
      } catch {
        setModsDir(null);
      }
    })();
  }, [config.gamePath]);

  async function saveKey() {
    setBusy(true);
    try {
      const u = await api.nexusSaveApiKey(apiKey.trim());
      onNexusUserChange(u);
      setApiKey("");
      const next = await api.getConfig();
      onConfigChange(next);
      const name = u.name ?? t("nexusUser");
      toast.success(
        u.isPremium
          ? t("connectedPremium", { name })
          : t("connectedFree", { name }),
      );
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function clearKey() {
    setBusy(true);
    try {
      await api.nexusClearApiKey();
      onNexusUserChange(null);
      setApiKey("");
      const next = await api.getConfig();
      onConfigChange(next);
      toast.success(t("apiKeyRemoved"));
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function changeGamePath() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("selectGameFolder"),
      });
      if (!selected || Array.isArray(selected)) return;
      setBusy(true);
      const next = await api.confirmGamePath(selected);
      onConfigChange(next);
      const paths = await api.getGamePaths();
      setModsDir(paths.modsDir);
      toast.success(t("gamePathUpdated"));
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function openModsFolder() {
    if (!modsDir) return;
    try {
      await openPath(modsDir);
    } catch (e) {
      toast.error(errMessage(e));
    }
  }

  return (
    <div>
      <h1 className="page-title">{t("settingsTitle")}</h1>
      <p className="page-desc">{t("settingsDesc")}</p>

      <section className="card" style={{ marginBottom: "1rem" }}>
        <h2 style={{ margin: "0 0 0.75rem", fontSize: "1.05rem" }}>{t("languageSection")}</h2>
        <p style={{ color: "var(--text-muted)", fontSize: "0.88rem", marginTop: 0 }}>
          {t("languageDesc")}
        </p>
        <div className="row-between" style={{ alignItems: "center" }}>
          <div>
            <div style={{ fontWeight: 650 }}>{t("oldEnglish")}</div>
            <div style={{ color: "var(--text-muted)", fontSize: "0.82rem", marginTop: 4 }}>
              {t("oldEnglishHint")}
            </div>
          </div>
          <label className="switch" title={t("oldEnglish")}>
            <input
              type="checkbox"
              checked={oldEnglish}
              onChange={(e) => setOldEnglish(e.target.checked)}
            />
            <span />
          </label>
        </div>
      </section>

      <section className="card" style={{ marginBottom: "1rem" }}>
        <h2 style={{ margin: "0 0 0.75rem", fontSize: "1.05rem" }}>{t("gameSection")}</h2>
        <div style={{ marginBottom: "0.65rem", color: "var(--text-muted)", fontSize: "0.88rem" }}>
          {t("installPath")}
        </div>
        <div className="path-box" style={{ marginBottom: "0.75rem" }}>
          {config.gamePath ?? t("notSet")}
        </div>
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "1rem" }}>
          <button className="btn btn-secondary btn-sm" disabled={busy} onClick={changeGamePath}>
            {t("changeFolder")}
          </button>
          <button className="btn btn-ghost btn-sm" disabled={!modsDir} onClick={openModsFolder}>
            {t("openModsFolder")}
          </button>
        </div>
        {modsDir && (
          <div>
            <div style={{ color: "var(--text-muted)", fontSize: "0.88rem", marginBottom: 6 }}>
              {t("modsDirectory")}
            </div>
            <div className="path-box">{modsDir}</div>
          </div>
        )}
      </section>

      <section className="card" style={{ marginBottom: "1rem" }}>
        <h2 style={{ margin: "0 0 0.5rem", fontSize: "1.05rem" }}>{t("nexusKeySection")}</h2>
        <p style={{ color: "var(--text-muted)", fontSize: "0.88rem", marginTop: 0 }}>
          {t("nexusKeyHelp")}{" "}
          <button
            className="btn-ghost"
            style={{
              display: "inline",
              padding: 0,
              color: "var(--accent)",
              cursor: "pointer",
              background: "none",
              border: "none",
              textDecoration: "underline",
            }}
            onClick={() =>
              void openUrl("https://www.nexusmods.com/users/myaccount?tab=api")
            }
          >
            {t("nexusApiLink")}
          </button>
        </p>
        <input
          className="input"
          type="password"
          placeholder={
            config.hasNexusApiKey ? t("replaceKey") : t("pasteKey")
          }
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          autoComplete="off"
          style={{ marginBottom: "0.75rem" }}
        />
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
          <button
            className="btn btn-primary"
            disabled={busy || !apiKey.trim()}
            onClick={saveKey}
          >
            {busy ? <span className="spinner" /> : null}
            {t("saveValidate")}
          </button>
          {config.hasNexusApiKey && (
            <button className="btn btn-secondary" disabled={busy} onClick={clearKey}>
              {t("removeKey")}
            </button>
          )}
        </div>
        {(nexusUser || config.hasNexusApiKey) && (
          <div style={{ marginTop: "0.85rem" }}>
            <span className="badge badge-ok">
              {nexusUser
                ? t("signedInAs", { name: nexusUser.name ?? t("user") })
                : t("keyStored")}
              {nexusUser?.isPremium
                ? t("nexusPremium")
                : nexusUser
                  ? t("freeNxm")
                  : ""}
            </span>
          </div>
        )}
      </section>

      <section className="card">
        <h2 style={{ margin: "0 0 0.5rem", fontSize: "1.05rem" }}>{t("aboutSection")}</h2>
        <p style={{ color: "var(--text-muted)", fontSize: "0.88rem", margin: 0 }}>
          {t("aboutBody")}
        </p>
        <button
          className="btn btn-ghost btn-sm"
          style={{ marginTop: "0.85rem" }}
          onClick={() => {
            if (confirm(t("confirmSetupAgain"))) {
              onResetSetup();
            }
          }}
        >
          {t("runSetupAgain")}
        </button>
      </section>
    </div>
  );
}
