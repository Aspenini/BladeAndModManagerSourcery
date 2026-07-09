import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  api,
  errMessage,
  NexusFile,
  NexusModDetail,
  NexusModSummary,
  NexusUser,
  onNxmDownload,
} from "../lib/tauri";
import { useI18n } from "../lib/i18n";
import { useToast } from "../lib/toast";

type Props = {
  hasApiKey: boolean;
  nexusUser: NexusUser | null;
  onGoSettings: () => void;
};

export default function Browse({ hasApiKey, nexusUser, onGoSettings }: Props) {
  const { t } = useI18n();
  const toast = useToast();
  const [sort, setSort] = useState<"trending" | "latest" | "updated">("trending");
  const [query, setQuery] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [mods, setMods] = useState<NexusModSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<NexusModDetail | null>(null);
  const [files, setFiles] = useState<NexusFile[]>([]);
  const [detailLoading, setDetailLoading] = useState(false);
  const [installingId, setInstallingId] = useState<number | null>(null);
  const [waitingForNxm, setWaitingForNxm] = useState(false);
  const listRequest = useRef(0);
  const detailRequest = useRef(0);

  const isPremium = Boolean(nexusUser?.isPremium);

  const loadList = useCallback(async () => {
    if (!hasApiKey) return;
    const request = ++listRequest.current;
    setLoading(true);
    try {
      const list = await api.nexusListMods(sort, query || undefined);
      if (request === listRequest.current) setMods(list);
    } catch (e) {
      if (request === listRequest.current) {
        toast.error(errMessage(e));
        setMods([]);
      }
    } finally {
      if (request === listRequest.current) setLoading(false);
    }
  }, [hasApiKey, sort, query, toast]);

  useEffect(() => {
    void loadList();
  }, [loadList]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onNxmDownload((ev) => {
      if (ev.stage === "started") {
        setWaitingForNxm(false);
        setInstallingId(ev.fileId || null);
      } else if (ev.stage === "finished" || ev.stage === "error") {
        setInstallingId(null);
        setWaitingForNxm(false);
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

  async function openDetail(modId: number) {
    const request = ++detailRequest.current;
    setSelectedId(modId);
    setDetail(null);
    setFiles([]);
    setDetailLoading(true);
    setWaitingForNxm(false);
    try {
      const [d, f] = await Promise.all([
        api.nexusModDetail(modId),
        api.nexusModFiles(modId),
      ]);
      if (request === detailRequest.current) {
        setDetail(d);
        setFiles(f);
      }
    } catch (e) {
      if (request === detailRequest.current) toast.error(errMessage(e));
    } finally {
      if (request === detailRequest.current) setDetailLoading(false);
    }
  }

  async function installFilePremium(file: NexusFile) {
    if (!detail) return;
    setInstallingId(file.fileId);
    setWaitingForNxm(false);
    try {
      const result = await api.nexusDownloadAndInstall(
        detail.modId,
        file.fileId,
        detail.name,
        file.version ?? undefined,
      );
      toast.success(t("installed", { name: result.folderName }));
    } catch (e) {
      toast.error(errMessage(e));
    } finally {
      setInstallingId(null);
    }
  }

  async function downloadThroughNexus(file: NexusFile) {
    if (!detail) return;
    try {
      const url = await api.nexusFileUrl(detail.modId, file.fileId);
      await openUrl(url);
      setWaitingForNxm(true);
      toast.info(t("waitingNexusToast"));
    } catch (e) {
      toast.error(errMessage(e));
      setWaitingForNxm(false);
    }
  }

  async function openOnNexus(modId: number) {
    try {
      const url = await api.nexusModUrl(modId);
      await openUrl(url);
    } catch (e) {
      toast.error(errMessage(e));
    }
  }

  if (!hasApiKey) {
    return (
      <div>
        <h1 className="page-title">{t("browseTitle")}</h1>
        <p className="page-desc">{t("connectApiDesc")}</p>
        <div className="card empty">
          <h3>{t("apiKeyRequiredTitle")}</h3>
          <p>{t("apiKeyRequiredBody")}</p>
          <button className="btn btn-primary" style={{ marginTop: "1rem" }} onClick={onGoSettings}>
            {t("openSettings")}
          </button>
        </div>
      </div>
    );
  }

  if (selectedId != null) {
    return (
      <div>
        <button
          className="btn btn-ghost btn-sm"
          style={{ marginBottom: "0.75rem" }}
          onClick={() => {
            detailRequest.current += 1;
            setSelectedId(null);
            setDetail(null);
            setFiles([]);
            setWaitingForNxm(false);
          }}
        >
          {t("backToResults")}
        </button>

        {waitingForNxm && (
          <div
            className="alert alert-info"
            style={{ display: "flex", gap: 10, alignItems: "center" }}
          >
            <span className="spinner" />
            <span>{t("waitingNexus")}</span>
          </div>
        )}

        {detailLoading ? (
          <div className="empty">
            <div className="spinner" style={{ margin: "0 auto 0.75rem" }} />
            {t("loadingMod")}
          </div>
        ) : !detail ? (
          <div className="card empty">
            <h3>{t("couldNotLoadMod")}</h3>
            <p>{t("checkConnection")}</p>
            <button className="btn btn-secondary" onClick={() => void openDetail(selectedId)}>
              {t("tryAgain")}
            </button>
          </div>
        ) : (
          <div className="detail-layout">
            <div>
              {detail.pictureUrl ? (
                <img
                  src={detail.pictureUrl}
                  alt=""
                  style={{
                    width: "100%",
                    borderRadius: 14,
                    border: "1px solid var(--border)",
                    aspectRatio: "1",
                    objectFit: "cover",
                  }}
                />
              ) : (
                <div
                  className="card"
                  style={{ height: 220, display: "grid", placeItems: "center" }}
                >
                  {t("noImage")}
                </div>
              )}
            </div>
            <div>
              <h1 className="page-title">{detail.name}</h1>
              <p className="page-desc" style={{ marginBottom: "0.75rem" }}>
                {detail.author
                  ? t("byAuthor", { name: detail.author })
                  : t("unknownAuthor")}
                {detail.version ? ` · v${detail.version}` : ""}
              </p>
              <div className="toolbar">
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => void openOnNexus(detail.modId)}
                >
                  {t("openOnNexus")}
                </button>
              </div>
              {detail.summary && (
                <div className="card" style={{ marginBottom: "1rem" }}>
                  <div
                    style={{ color: "var(--text-muted)", fontSize: "0.9rem", lineHeight: 1.45 }}
                  >
                    {toPlainText(detail.summary)}
                  </div>
                </div>
              )}

              {!isPremium && (
                <div
                  className="card"
                  style={{
                    marginBottom: "1rem",
                    fontSize: "0.88rem",
                    color: "var(--text-muted)",
                    lineHeight: 1.45,
                  }}
                >
                  {t("freeAccountHint")}
                </div>
              )}

              <h2 style={{ fontSize: "1.05rem", margin: "0 0 0.65rem" }}>{t("files")}</h2>
              {files.length === 0 ? (
                <div className="card empty" style={{ padding: "1.5rem" }}>
                  <p>{t("noFiles")}</p>
                </div>
              ) : (
                <div className="card" style={{ padding: 0 }}>
                  {files.map((file) => (
                    <div className="list-row" key={file.fileId}>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ fontWeight: 650 }}>
                          {file.name}
                          {file.isPrimary && (
                            <>
                              {" "}
                              <span className="badge badge-accent">{t("mainFile")}</span>
                            </>
                          )}
                        </div>
                        <div className="mod-meta">
                          {file.version ? `v${file.version} · ` : ""}
                          {file.categoryName ? `${file.categoryName} · ` : ""}
                          {file.sizeKb != null
                            ? `${Math.max(1, Math.round(file.sizeKb / 1024))} MB`
                            : ""}
                        </div>
                      </div>
                      {isPremium ? (
                        <button
                          className="btn btn-primary btn-sm"
                          disabled={installingId !== null}
                          onClick={() => void installFilePremium(file)}
                        >
                          {installingId === file.fileId ? (
                            <span className="spinner" />
                          ) : (
                            t("install")
                          )}
                        </button>
                      ) : (
                        <button
                          className="btn btn-primary btn-sm"
                          disabled={installingId !== null}
                          onClick={() => void downloadThroughNexus(file)}
                          title={t("downloadThroughNexusTitle")}
                        >
                          {installingId === file.fileId ? (
                            <span className="spinner" />
                          ) : (
                            t("downloadThroughNexus")
                          )}
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div>
      <h1 className="page-title">{t("browseTitle")}</h1>
      <p className="page-desc">{t("browseDesc")}</p>

      <div className="toolbar">
        <form
          style={{ display: "flex", gap: 8, flex: 1, minWidth: 220 }}
          onSubmit={(e) => {
            e.preventDefault();
            const nextQuery = searchInput.trim();
            if (nextQuery === query) {
              void loadList();
            } else {
              setQuery(nextQuery);
            }
          }}
        >
          <input
            className="input"
            placeholder={t("searchMods")}
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
          />
          <button className="btn btn-primary" type="submit">
            {t("search")}
          </button>
        </form>
        <select
          className="input"
          style={{ width: "auto" }}
          value={sort}
          onChange={(e) =>
            setSort(e.target.value as "trending" | "latest" | "updated")
          }
        >
          <option value="trending">{t("sortTrending")}</option>
          <option value="latest">{t("sortLatest")}</option>
          <option value="updated">{t("sortUpdated")}</option>
        </select>
        <button className="btn btn-ghost" onClick={() => void loadList()}>
          {t("refresh")}
        </button>
      </div>

      {loading ? (
        <div className="empty">
          <div className="spinner" style={{ margin: "0 auto 0.75rem" }} />
          {t("loadingFromNexus")}
        </div>
      ) : mods.length === 0 ? (
        <div className="card empty">
          <h3>{t("noModsFound")}</h3>
          <p>{t("tryAnotherSearch")}</p>
        </div>
      ) : (
        <div className="mod-grid">
          {mods.map((mod) => (
            <button
              key={mod.modId}
              className="mod-card"
              style={{
                textAlign: "left",
                cursor: "pointer",
                color: "inherit",
                padding: 0,
                font: "inherit",
              }}
              onClick={() => void openDetail(mod.modId)}
            >
              {mod.pictureUrl ? (
                <img className="mod-thumb" src={mod.pictureUrl} alt="" />
              ) : (
                <div className="mod-thumb" />
              )}
              <div className="mod-body">
                <div className="mod-name">{mod.name}</div>
                <div className="mod-meta">
                  {mod.author ?? t("unknown")}
                  {mod.version ? ` · v${mod.version}` : ""}
                </div>
                {mod.summary && (
                  <div className="mod-summary">{toPlainText(mod.summary)}</div>
                )}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function toPlainText(html: string): string {
  return html
    .replace(/<[^>]*>/g, " ")
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&quot;/gi, '"')
    .replace(/&#39;|&apos;/gi, "'")
    .replace(/\s+/g, " ")
    .trim();
}
