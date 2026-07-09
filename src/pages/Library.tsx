import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errMessage, LocalMod } from "../lib/tauri";
import { useI18n } from "../lib/i18n";
import { useToast } from "../lib/toast";

type Props = {
  onBrowse: () => void;
};

export default function Library({ onBrowse }: Props) {
  const { t } = useI18n();
  const toast = useToast();
  const [mods, setMods] = useState<LocalMod[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [visibility, setVisibility] = useState<"all" | "enabled" | "disabled">("all");

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const list = await api.listMods();
      setMods(list);
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function onToggle(mod: LocalMod) {
    setBusyId(mod.folderName);
    try {
      await api.toggleMod(mod.folderName, !mod.enabled);
      await refresh();
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBusyId(null);
    }
  }

  async function onRemove(mod: LocalMod) {
    if (!confirm(t("confirmUninstall", { name: mod.displayName }))) {
      return;
    }
    setBusyId(mod.folderName);
    try {
      await api.removeMod(mod.folderName);
      toast.success(t("removed", { name: mod.displayName }));
      await refresh();
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBusyId(null);
    }
  }

  async function onImport() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: t("modArchive"), extensions: ["zip"] }],
        title: t("importTitle"),
      });
      if (!selected || Array.isArray(selected)) return;
      setBusyId("__import__");
      const folder = await api.importArchive(selected);
      toast.success(t("installed", { name: folder }));
      await refresh();
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBusyId(null);
    }
  }

  async function reveal(mod: LocalMod) {
    try {
      await revealItemInDir(mod.path);
    } catch (e) {
      toast.error(errMessage(e));
    }
  }

  const shownMods = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return mods.filter((mod) => {
      const matchesVisibility =
        visibility === "all" ||
        (visibility === "enabled" ? mod.enabled : !mod.enabled);
      const matchesSearch =
        !needle ||
        [mod.displayName, mod.folderName, mod.author, mod.version]
          .filter(Boolean)
          .some((value) => value!.toLocaleLowerCase().includes(needle));
      return matchesVisibility && matchesSearch;
    });
  }, [mods, query, visibility]);

  const enabledCount = mods.filter((mod) => mod.enabled).length;

  return (
    <div>
      <div className="row-between" style={{ alignItems: "flex-start" }}>
        <div>
          <h1 className="page-title">{t("libraryTitle")}</h1>
          <p className="page-desc">{t("libraryDesc")}</p>
        </div>
      </div>

      <div className="toolbar">
        <button className="btn btn-primary" onClick={onBrowse}>
          {t("browseNexus")}
        </button>
        <button
          className="btn btn-secondary"
          onClick={onImport}
          disabled={busyId === "__import__"}
        >
          {busyId === "__import__" ? <span className="spinner" /> : null}
          {t("importZip")}
        </button>
        <button className="btn btn-ghost" onClick={() => void refresh()}>
          {t("refresh")}
        </button>
      </div>

      {loading ? (
        <div className="empty">
          <div className="spinner" style={{ margin: "0 auto 0.75rem" }} />
          {t("loadingMods")}
        </div>
      ) : mods.length === 0 ? (
        <div className="card empty">
          <h3>{t("noModsTitle")}</h3>
          <p>{t("noModsBody")}</p>
          <div style={{ marginTop: "1rem", display: "flex", gap: "0.5rem", justifyContent: "center" }}>
            <button className="btn btn-primary" onClick={onBrowse}>
              {t("browseNexus")}
            </button>
            <button className="btn btn-secondary" onClick={onImport}>
              {t("importZip")}
            </button>
          </div>
        </div>
      ) : (
        <>
          <div className="library-tools">
            <input
              className="input"
              placeholder={t("filterMods")}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              aria-label={t("filterMods")}
            />
            <div className="filter-group" aria-label={t("filterMods")}>
              {(["all", "enabled", "disabled"] as const).map((option) => (
                <button
                  key={option}
                  className={`filter-btn ${visibility === option ? "active" : ""}`}
                  onClick={() => setVisibility(option)}
                >
                  {option === "all"
                    ? t("filterAll", { n: mods.length })
                    : option === "enabled"
                      ? t("filterEnabled", { n: enabledCount })
                      : t("filterDisabled", { n: mods.length - enabledCount })}
                </button>
              ))}
            </div>
          </div>
          {shownMods.length === 0 ? (
            <div className="card empty" style={{ padding: "1.5rem" }}>
              <h3>{t("noMatchTitle")}</h3>
              <p>{t("noMatchBody")}</p>
            </div>
          ) : (
            <div className="card" style={{ padding: 0, overflow: "hidden" }}>
              {shownMods.map((mod) => (
                <div className="list-row" key={mod.folderName}>
                  <label
                    className="switch"
                    title={mod.enabled ? t("enabled") : t("disabled")}
                  >
                    <input
                      type="checkbox"
                      checked={mod.enabled}
                      disabled={busyId === mod.folderName}
                      onChange={() => void onToggle(mod)}
                    />
                    <span />
                  </label>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontWeight: 700 }}>{mod.displayName}</div>
                    <div className="mod-meta">
                      {mod.author ? `${mod.author} · ` : ""}
                      {mod.version ? `v${mod.version} · ` : ""}
                      <span style={{ opacity: 0.85 }}>{mod.folderName}</span>
                      {!mod.hasManifest && (
                        <>
                          {" · "}
                          <span className="badge badge-muted">{t("noManifest")}</span>
                        </>
                      )}
                    </div>
                    {mod.description && (
                      <div className="mod-summary" style={{ marginTop: 4 }}>
                        {mod.description}
                      </div>
                    )}
                  </div>
                  <button className="btn btn-ghost btn-sm" onClick={() => void reveal(mod)}>
                    {t("open")}
                  </button>
                  <button
                    className="btn btn-danger btn-sm"
                    disabled={busyId === mod.folderName}
                    onClick={() => void onRemove(mod)}
                  >
                    {t("uninstall")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
