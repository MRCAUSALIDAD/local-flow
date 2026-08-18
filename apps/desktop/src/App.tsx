import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TitleBar } from "./components/TitleBar";
import { StatusOrb } from "./components/StatusOrb";
import { TranscriptPanel } from "./components/TranscriptPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { LivePanel } from "./components/LivePanel";
import * as api from "./lib/api";
import type {
  Config,
  FlowStatus,
  LiveEntry,
  LiveState,
  LoopbackSource,
  ModelInfo,
  TranscriptEntry,
} from "./lib/types";
import "./App.css";

type Tab = "home" | "listen" | "settings";

const DEFAULT_CONFIG: Config = {
  model_path: null,
  input_device: null,
  language: "auto",
  auto_type: true,
  copy_to_clipboard: true,
  show_metrics: false,
  metrics_corner: "top-right",
  loopback_source: null,
  capture_mic: true,
  vad_silence_ms: 600,
  live_max_chunk_secs: 25,
};

function App() {
  const [tab, setTab] = useState<Tab>("home");
  const [status, setStatus] = useState<FlowStatus>("idle");
  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG);
  const [hasModel, setHasModel] = useState(false);
  const [microphones, setMicrophones] = useState<string[]>([]);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState<{ id: string; pct: number } | null>(
    null,
  );
  const [entries, setEntries] = useState<TranscriptEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [debug, setDebug] = useState<string | null>(null);
  const [accessOk, setAccessOk] = useState(true);
  const [liveEntries, setLiveEntries] = useState<LiveEntry[]>([]);
  const [liveState, setLiveState] = useState<LiveState | null>(null);
  const [sources, setSources] = useState<LoopbackSource[]>([]);
  const [systemAudioError, setSystemAudioError] = useState<string | null>(null);
  const idRef = useRef(0);

  async function refreshModels() {
    setModels(await api.listModels().catch(() => []));
    setHasModel(await api.modelLoaded().catch(() => false));
    setConfig(await api.getConfig().catch((): Config => config));
  }

  useEffect(() => {
    api.getConfig().then(setConfig).catch(() => {});
    api.modelLoaded().then(setHasModel).catch(() => {});
    api.listMicrophones().then(setMicrophones).catch(() => {});
    api.listModels().then(setModels).catch(() => {});
    api.accessibilityOk().then(setAccessOk).catch(() => {});
    api.systemAudioStatus().then(setSystemAudioError).catch(() => {});
    api.listLoopbackSources().then(setSources).catch(() => {});
    api.liveEntries().then(setLiveEntries).catch(() => {});

    const poll = setInterval(() => {
      api.accessibilityOk().then(setAccessOk).catch(() => {});
    }, 2500);

    const unlisten = Promise.all([
      listen<FlowStatus>("flow-status", (e) => setStatus(e.payload)),
      listen<string>("flow-transcript", (e) => {
        setEntries((prev) => [
          { id: idRef.current++, text: e.payload, at: Date.now() },
          ...prev,
        ]);
        setError(null);
      }),
      listen<string>("flow-error", (e) => {
        setError(e.payload);
        setDownloading(null);
      }),
      listen<string>("flow-debug", (e) => setDebug(e.payload)),
      listen<[string, number, number]>("model-progress", (e) => {
        const [id, received, total] = e.payload;
        const pct = total > 0 ? Math.round((received / total) * 100) : 0;
        setDownloading({ id, pct });
      }),
      listen<string>("model-ready", () => {
        setDownloading(null);
        refreshModels();
      }),
      listen<LiveEntry>("flow-live-segment", (e) => {
        // Segments can arrive out of order: both tracks share one
        // transcription queue, so a short utterance can finish after a
        // longer, earlier one. Insert by start time to match the backend.
        setLiveEntries((prev) => {
          const next = [...prev];
          const at = next.findIndex((x) => x.start_ms > e.payload.start_ms);
          if (at === -1) next.push(e.payload);
          else next.splice(at, 0, e.payload);
          return next;
        });
      }),
      listen<LiveState>("flow-live-state", (e) => setLiveState(e.payload)),
    ]);

    return () => {
      clearInterval(poll);
      unlisten.then((fns) => fns.forEach((f) => f()));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function persist(next: Config) {
    setConfig(next);
    setError(null);
    try {
      await api.setConfig(next);
      setHasModel(await api.modelLoaded());
    } catch (e) {
      setError(String(e));
      setHasModel(await api.modelLoaded());
    }
  }

  function onDownload(id: string) {
    setError(null);
    setDownloading({ id, pct: 0 });
    api.downloadModel(id).catch((e) => {
      setError(String(e));
      setDownloading(null);
    });
  }

  return (
    <div className="app">
      <div className="app__glow" />
      <TitleBar />

      <nav className="tabs">
        <button
          className={`tab ${tab === "home" ? "tab--active" : ""}`}
          onClick={() => setTab("home")}
        >
          Dictate
        </button>
        <button
          className={`tab ${tab === "listen" ? "tab--active" : ""}`}
          onClick={() => setTab("listen")}
        >
          Listen
        </button>
        <button
          className={`tab ${tab === "settings" ? "tab--active" : ""}`}
          onClick={() => setTab("settings")}
        >
          Settings
        </button>
        <span className={`status-dot status-dot--${hasModel ? "ok" : "off"}`}>
          {hasModel ? "Model ready" : "No model"}
        </span>
      </nav>

      <main className="app__body">
        {tab === "home" ? (
          <>
            <StatusOrb
              status={status}
              disabled={!hasModel}
              onPressStart={() => api.startDictation()}
              onPressEnd={() => api.stopDictation()}
            />
            {!hasModel && (
              <p className="banner banner--info" onClick={() => setTab("settings")}>
                Download a voice model in Settings to start dictating →
              </p>
            )}
            {!accessOk && (
              <div className="banner banner--warn">
                <span>
                  To type into other apps, Local Flow needs{" "}
                  <b>Accessibility</b> permission.
                </span>
                <div className="banner__actions">
                  <button
                    className="ghost-btn"
                    onClick={() => api.requestAccessibility()}
                  >
                    Grant
                  </button>
                  <button
                    className="ghost-btn"
                    onClick={() => api.openAccessibilitySettings()}
                  >
                    Open Settings
                  </button>
                </div>
              </div>
            )}
            {error && <p className="banner banner--error">{error}</p>}
            {debug && (
              <p className="banner banner--info" onClick={() => setDebug(null)}>
                debug: {debug}
              </p>
            )}
            <TranscriptPanel entries={entries} onClear={() => setEntries([])} />
          </>
        ) : tab === "listen" ? (
          <LivePanel
            config={config}
            onChange={persist}
            entries={liveEntries}
            state={liveState}
            sources={sources}
            unavailable={systemAudioError}
            hasModel={hasModel}
            onClear={() => {
              api.clearSession().catch(() => {});
              setLiveEntries([]);
            }}
          />
        ) : (
          <SettingsPanel
            config={config}
            onChange={persist}
            error={error}
            microphones={microphones}
            models={models}
            downloading={downloading}
            onDownload={onDownload}
          />
        )}
      </main>

      <footer className="app__foot">
        <span>Offline · Private · Local Flow</span>
      </footer>
    </div>
  );
}

export default App;
