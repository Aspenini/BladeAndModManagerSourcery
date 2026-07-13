import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, BoxesState, errMessage, LocalMod } from "../lib/tauri";
import { useI18n } from "../lib/i18n";
import { useToast } from "../lib/toast";

type Props = {
  onBrowse: () => void;
};

type BoxFilter = "all" | "unboxed" | string;

export default function Library({ onBrowse }: Props) {
  const { t } = useI18n();
  const toast = useToast();
  const [mods, setMods] = useState<LocalMod[]>([]);
  const [boxes, setBoxes] = useState<BoxesState>({
    boxes: [],
    activeBoxId: null,
    assignments: {},
  });
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [boxBusy, setBoxBusy] = useState(false);
  const [query, setQuery] = useState("");
  const [visibility, setVisibility] = useState<"all" | "enabled" | "disabled">("all");
  const [boxFilter, setBoxFilter] = useState<BoxFilter>("all");
  const [newBoxName, setNewBoxName] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [list, boxState] = await Promise.all([api.listMods(), api.listBoxes()]);
      setMods(list);
      setBoxes(boxState);
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

  async function onAssignBox(mod: LocalMod, boxId: string) {
    try {
      const next = await api.assignModBox(mod.folderName, boxId || null);
      setBoxes(next);
      setMods((prev) =>
        prev.map((m) =>
          m.folderName === mod.folderName ? { ...m, boxId: boxId || null } : m,
        ),
      );
    } catch (e) {
      toast.error(errMessage(e));
    }
  }

  async function onCreateBox() {
    const name = newBoxName.trim();
    if (!name) return;
    setBoxBusy(true);
    try {
      const next = await api.createBox(name);
      setBoxes(next);
      setNewBoxName("");
      const created = next.boxes.find((b) => b.name === name);
      if (created) setBoxFilter(created.id);
      toast.success(t("boxCreated", { name }));
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBoxBusy(false);
    }
  }

  async function onRenameBox(boxId: string) {
    const name = newBoxName.trim();
    if (!name) {
      toast.info(t("renameNeedsName"));
      return;
    }
    setBoxBusy(true);
    try {
      const next = await api.renameBox(boxId, name);
      setBoxes(next);
      setNewBoxName("");
      toast.success(t("boxRenamed", { name }));
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBoxBusy(false);
    }
  }

  async function onDeleteBox(boxId: string) {
    const box = boxes.boxes.find((b) => b.id === boxId);
    if (!box) return;
    if (!confirm(t("confirmDeleteBox", { name: box.name }))) return;
    setBoxBusy(true);
    try {
      const next = await api.deleteBox(boxId);
      setBoxes(next);
      setBoxFilter("all");
      toast.success(t("boxDeleted", { name: box.name }));
      await refresh();
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBoxBusy(false);
    }
  }

  async function onActivateBox(boxId: string) {
    setBoxBusy(true);
    try {
      const report = await api.activateBox(boxId);
      toast.success(
        t("boxActivated", {
          name: report.boxName,
          enabled: report.enabled,
          disabled: report.disabled,
        }),
      );
      if (report.errors.length > 0 || report.skipped.length > 0) {
        toast.error(
          t("boxActivationIssues", {
            issues: [...report.errors, ...report.skipped].join(", "),
          }),
        );
      }
      await refresh();
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBoxBusy(false);
    }
  }

  async function onDeactivateBox() {
    setBoxBusy(true);
    try {
      const next = await api.clearActiveBox();
      setBoxes(next);
      toast.info(t("boxDeactivated"));
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setBoxBusy(false);
    }
  }

  const boxCounts = useMemo(() => {
    const counts = new Map<string, number>();
    let unboxed = 0;
    for (const mod of mods) {
      if (mod.boxId) {
        counts.set(mod.boxId, (counts.get(mod.boxId) ?? 0) + 1);
      } else {
        unboxed += 1;
      }
    }
    return { counts, unboxed };
  }, [mods]);

  const shownMods = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return mods.filter((mod) => {
      const matchesBox =
        boxFilter === "all" ||
        (boxFilter === "unboxed" ? !mod.boxId : mod.boxId === boxFilter);
      const matchesVisibility =
        visibility === "all" ||
        (visibility === "enabled" ? mod.enabled : !mod.enabled);
      const matchesSearch =
        !needle ||
        [mod.displayName, mod.folderName, mod.author, mod.version, mod.gameVersion]
          .filter(Boolean)
          .some((value) => value!.toLocaleLowerCase().includes(needle));
      return matchesBox && matchesVisibility && matchesSearch;
    });
  }, [mods, query, visibility, boxFilter]);

  const enabledCount = mods.filter((mod) => mod.enabled).length;
  const selectedBox =
    boxFilter !== "all" && boxFilter !== "unboxed"
      ? boxes.boxes.find((b) => b.id === boxFilter) ?? null
      : null;

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

      <div className="card box-bar">
        <div className="box-bar-head">
          <h2 style={{ margin: 0 }}>{t("boxesTitle")}</h2>
          <p className="page-desc" style={{ margin: "0.25rem 0 0" }}>
            {t("boxesDesc")}
          </p>
        </div>
        <div className="box-bar-row">
          <div className="filter-group">
            <button
              className={`filter-btn ${boxFilter === "all" ? "active" : ""}`}
              onClick={() => setBoxFilter("all")}
            >
              {t("boxAll", { n: mods.length })}
            </button>
            {boxes.boxes.map((box) => (
              <button
                key={box.id}
                className={`filter-btn ${boxFilter === box.id ? "active" : ""}`}
                onClick={() => setBoxFilter(box.id)}
              >
                {box.name} ({boxCounts.counts.get(box.id) ?? 0})
                {boxes.activeBoxId === box.id && (
                  <span className="badge badge-accent box-active-badge">
                    {t("activeBadge")}
                  </span>
                )}
              </button>
            ))}
            {boxCounts.unboxed > 0 && (
              <button
                className={`filter-btn ${boxFilter === "unboxed" ? "active" : ""}`}
                onClick={() => setBoxFilter("unboxed")}
              >
                {t("boxUnboxed", { n: boxCounts.unboxed })}
              </button>
            )}
          </div>
        </div>
        <div className="box-bar-row">
          <input
            className="input"
            style={{ maxWidth: 260 }}
            placeholder={t("newBoxPlaceholder")}
            value={newBoxName}
            onChange={(event) => setNewBoxName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void onCreateBox();
            }}
          />
          <button
            className="btn btn-secondary btn-sm"
            disabled={boxBusy || !newBoxName.trim()}
            onClick={() => void onCreateBox()}
          >
            {t("createBox")}
          </button>
          {selectedBox && (
            <>
              <span className="box-actions-divider" aria-hidden />
              {boxes.activeBoxId === selectedBox.id ? (
                <button
                  className="btn btn-ghost btn-sm"
                  disabled={boxBusy}
                  onClick={() => void onDeactivateBox()}
                >
                  {t("deactivateBox")}
                </button>
              ) : (
                <button
                  className="btn btn-primary btn-sm"
                  disabled={boxBusy}
                  onClick={() => void onActivateBox(selectedBox.id)}
                >
                  {boxBusy ? <span className="spinner" /> : null}
                  {t("activateBox")} “{selectedBox.name}”
                </button>
              )}
              <button
                className="btn btn-ghost btn-sm"
                disabled={boxBusy}
                onClick={() => void onRenameBox(selectedBox.id)}
              >
                {t("renameBox")}
              </button>
              <button
                className="btn btn-danger btn-sm"
                disabled={boxBusy}
                onClick={() => void onDeleteBox(selectedBox.id)}
              >
                {t("deleteBox")}
              </button>
            </>
          )}
        </div>
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
                    title={
                      !mod.hasManifest
                        ? t("cannotToggleNoManifest")
                        : mod.enabled
                          ? t("enabled")
                          : t("disabled")
                    }
                  >
                    <input
                      type="checkbox"
                      checked={mod.enabled}
                      disabled={busyId === mod.folderName || !mod.hasManifest}
                      onChange={() => void onToggle(mod)}
                    />
                    <span />
                  </label>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontWeight: 700 }}>
                      {mod.displayName}
                      {mod.gameVersion && (
                        <span
                          className="badge badge-accent"
                          style={{ marginLeft: 8 }}
                        >
                          {t("forGameVersion", { v: mod.gameVersion })}
                        </span>
                      )}
                    </div>
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
                  <select
                    className="input box-select"
                    value={mod.boxId ?? ""}
                    aria-label={t("boxSelectLabel", { name: mod.displayName })}
                    onChange={(event) => void onAssignBox(mod, event.target.value)}
                  >
                    <option value="">{t("noBox")}</option>
                    {boxes.boxes.map((box) => (
                      <option key={box.id} value={box.id}>
                        {box.name}
                      </option>
                    ))}
                  </select>
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
