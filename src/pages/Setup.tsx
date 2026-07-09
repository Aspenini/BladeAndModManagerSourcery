import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, DetectedGame, errMessage } from "../lib/tauri";
import { useI18n } from "../lib/i18n";

type Props = {
  onComplete: () => void;
};

export default function Setup({ onComplete }: Props) {
  const { t } = useI18n();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [detected, setDetected] = useState<DetectedGame | null>(null);
  const [manualPath, setManualPath] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await api.detectGame();
        if (!cancelled) setDetected(result);
      } catch (e) {
        if (!cancelled) setError(errMessage(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const active = manualPath
    ? {
        path: manualPath,
        source: "manual",
        valid: true,
        message: t("selectedPath"),
      }
    : detected;

  async function browse() {
    setError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("selectGameFolder"),
      });
      if (!selected || Array.isArray(selected)) return;
      setBusy(true);
      const inspected = await api.validatePath(selected);
      setDetected(inspected);
      setManualPath(inspected.path);
    } catch (e) {
      setError(errMessage(e));
      setManualPath(null);
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    if (!active?.path) return;
    setBusy(true);
    setError(null);
    try {
      await api.confirmGamePath(active.path);
      onComplete();
    } catch (e) {
      setError(errMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="setup-wrap">
      <div className="setup-card">
        <div className="setup-steps">
          <div className="setup-step on" />
          <div className={`setup-step ${active?.valid ? "on" : ""}`} />
          <div className="setup-step" />
        </div>

        <h1 className="page-title">{t("findGame")}</h1>
        <p className="page-desc">{t("findGameDesc")}</p>

        {error && <div className="alert alert-error">{error}</div>}

        {loading ? (
          <div className="empty">
            <div className="spinner" style={{ margin: "0 auto 0.75rem" }} />
            {t("scanning")}
          </div>
        ) : active?.valid ? (
          <div className="card" style={{ marginBottom: "1rem" }}>
            <div className="row-between" style={{ marginBottom: "0.65rem" }}>
              <strong>{t("isThisInstall")}</strong>
              <span className="badge badge-accent">
                {active.source === "steam"
                  ? t("steam")
                  : active.source === "oculus"
                    ? t("oculus")
                    : t("manual")}
              </span>
            </div>
            <div className="path-box">{active.path}</div>
          </div>
        ) : (
          <div className="alert alert-info">{t("couldNotFind")}</div>
        )}

        <div style={{ display: "flex", gap: "0.65rem", flexWrap: "wrap" }}>
          {active?.valid && (
            <button
              className="btn btn-primary"
              disabled={busy}
              onClick={confirm}
            >
              {busy ? <span className="spinner" /> : null}
              {t("yesUseFolder")}
            </button>
          )}
          <button className="btn btn-secondary" disabled={busy} onClick={browse}>
            {t("chooseDifferent")}
          </button>
        </div>

        <p
          style={{
            marginTop: "1.25rem",
            color: "var(--text-muted)",
            fontSize: "0.8rem",
            lineHeight: 1.45,
          }}
        >
          {t("typicalPath")}{" "}
          <code>…\steamapps\common\Blade &amp; Sorcery</code>
          <br />
          {t("modsInstallTo")}{" "}
          <code>BladeAndSorcery_Data\StreamingAssets\Mods</code>
        </p>
      </div>
    </div>
  );
}
