import { useCallback, useEffect, useRef, useState } from "react";
import { api, AppConfig, errMessage, NexusUser, onNxmDownload } from "./lib/tauri";
import { useI18n } from "./lib/i18n";
import { useToast } from "./lib/toast";
import Setup from "./pages/Setup";
import Library from "./pages/Library";
import Browse from "./pages/Browse";
import Settings from "./pages/Settings";

type Page = "library" | "browse" | "settings";

export default function App() {
  const { t } = useI18n();
  const toast = useToast();
  const toastRef = useRef(toast);
  toastRef.current = toast;
  const tRef = useRef(t);
  tRef.current = t;
  const [booting, setBooting] = useState(true);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [page, setPage] = useState<Page>("library");
  const [error, setError] = useState<string | null>(null);
  const [nexusUser, setNexusUser] = useState<NexusUser | null>(null);

  const load = useCallback(async () => {
    try {
      const c = await api.getConfig();
      setConfig(c);
      if (c.hasNexusApiKey) {
        try {
          const user = await api.nexusGetUser();
          setNexusUser(user);
        } catch {
          setNexusUser(null);
        }
      } else {
        setNexusUser(null);
      }
    } catch (e) {
      setError(errMessage(e));
    } finally {
      setBooting(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onNxmDownload((ev) => {
      if (ev.stage === "started") {
        setPage("browse");
        toastRef.current.info(
          ev.message ||
            tRef.current("downloadingMod", {
              modId: ev.modId,
              fileId: ev.fileId,
            }),
        );
      } else if (ev.stage === "finished") {
        toastRef.current.success(
          ev.folderName
            ? tRef.current("installed", { name: ev.folderName })
            : ev.message || tRef.current("modInstalled"),
        );
      } else if (ev.stage === "error") {
        toastRef.current.error(ev.message);
        setPage("browse");
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (booting) {
    return (
      <div className="setup-wrap">
        <div className="empty">
          <div className="spinner" style={{ margin: "0 auto 0.75rem" }} />
          {t("starting")}
        </div>
      </div>
    );
  }

  if (error && !config) {
    return (
      <div className="setup-wrap">
        <div className="setup-card">
          <div className="alert alert-error">{error}</div>
          <button className="btn btn-primary" onClick={() => void load()}>
            {t("retry")}
          </button>
        </div>
      </div>
    );
  }

  if (!config?.setupComplete || !config.gamePath) {
    return (
      <Setup
        onComplete={() => {
          void load();
        }}
      />
    );
  }

  const hasApiKey = Boolean(config.hasNexusApiKey);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <button
          className={`nav-btn ${page === "library" ? "active" : ""}`}
          onClick={() => setPage("library")}
        >
          {t("navLibrary")}
        </button>
        <button
          className={`nav-btn ${page === "browse" ? "active" : ""}`}
          onClick={() => setPage("browse")}
        >
          {t("navBrowse")}
        </button>
        <button
          className={`nav-btn ${page === "settings" ? "active" : ""}`}
          onClick={() => setPage("settings")}
        >
          {t("navSettings")}
        </button>

        <div style={{ flex: 1 }} />
      </aside>

      <div className="main">
        <header className="topbar">
          <div>
            {page === "library" && t("topbarLibrary")}
            {page === "browse" && t("topbarBrowse")}
            {page === "settings" && t("topbarSettings")}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            {hasApiKey ? (
              <span
                className={`status-seal ${
                  nexusUser?.isPremium
                    ? "status-seal-premium"
                    : nexusUser
                      ? "status-seal-free"
                      : "status-seal-bound"
                }`}
              >
                <span className="status-seal-mark" aria-hidden>
                  {nexusUser?.isPremium ? "⚜" : nexusUser ? "❧" : "✦"}
                </span>
                <span className="status-seal-text">
                  {t("nexusConnected")}
                  {nexusUser?.isPremium
                    ? t("nexusPremium")
                    : nexusUser
                      ? t("nexusFree")
                      : ""}
                </span>
              </span>
            ) : (
              <button
                type="button"
                className="status-seal status-seal-missing"
                onClick={() => setPage("settings")}
              >
                <span className="status-seal-mark" aria-hidden>
                  ✎
                </span>
                <span className="status-seal-text">{t("nexusKeyMissing")}</span>
              </button>
            )}
          </div>
        </header>
        <main className="content">
          {page === "library" && (
            <Library onBrowse={() => setPage("browse")} />
          )}
          {page === "browse" && (
            <Browse
              hasApiKey={hasApiKey}
              nexusUser={nexusUser}
              onGoSettings={() => setPage("settings")}
            />
          )}
          {page === "settings" && (
            <Settings
              config={config}
              nexusUser={nexusUser}
              onConfigChange={setConfig}
              onNexusUserChange={setNexusUser}
              onResetSetup={async () => {
                const next = {
                  ...config,
                  setupComplete: false,
                  gamePath: null,
                };
                await api.saveAppConfig(next);
                setConfig(next);
              }}
            />
          )}
        </main>
      </div>
    </div>
  );
}
