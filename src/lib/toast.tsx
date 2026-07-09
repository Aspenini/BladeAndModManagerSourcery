import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

export type ToastKind = "success" | "error" | "info";

export type Toast = {
  id: number;
  kind: ToastKind;
  message: string;
};

type ToastApi = {
  push: (message: string, kind?: ToastKind) => void;
  success: (message: string) => void;
  error: (message: string) => void;
  info: (message: string) => void;
  dismiss: (id: number) => void;
};

const ToastContext = createContext<ToastApi | null>(null);

const DEFAULT_MS = 5000;
/** Ignore identical toasts that fire back-to-back (StrictMode, double listeners, etc.). */
const DEDUPE_MS = 900;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const idRef = useRef(1);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const lastPush = useRef<{ key: string; at: number } | null>(null);

  const dismiss = useCallback((id: number) => {
    const t = timers.current.get(id);
    if (t) {
      clearTimeout(t);
      timers.current.delete(id);
    }
    setToasts((list) => list.filter((x) => x.id !== id));
  }, []);

  const push = useCallback(
    (message: string, kind: ToastKind = "info") => {
      const text = message.trim();
      if (!text) return;

      const key = `${kind}::${text}`;
      const now = Date.now();
      const prev = lastPush.current;
      if (prev && prev.key === key && now - prev.at < DEDUPE_MS) {
        return;
      }
      lastPush.current = { key, at: now };

      const id = idRef.current++;
      setToasts((list) => [...list.slice(-4), { id, kind, message: text }]);
      const timer = setTimeout(() => dismiss(id), DEFAULT_MS);
      timers.current.set(id, timer);
    },
    [dismiss],
  );

  // Stable API identity so consumers' useEffects don't re-subscribe every render.
  const api = useMemo<ToastApi>(
    () => ({
      push,
      success: (m) => push(m, "success"),
      error: (m) => push(m, "error"),
      info: (m) => push(m, "info"),
      dismiss,
    }),
    [push, dismiss],
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="toast-stack" aria-live="polite" aria-relevant="additions">
        {toasts.map((toast) => (
          <div
            key={toast.id}
            className={`toast toast-${toast.kind}`}
            role={toast.kind === "error" ? "alert" : "status"}
          >
            <span className="toast-msg">{toast.message}</span>
            <button
              type="button"
              className="toast-close"
              aria-label="Dismiss"
              onClick={() => dismiss(toast.id)}
            >
              ×
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error("useToast must be used within ToastProvider");
  }
  return ctx;
}
