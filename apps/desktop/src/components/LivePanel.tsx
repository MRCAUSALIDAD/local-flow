import { useEffect, useRef, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import * as api from "../lib/api";
import type {
  Config,
  ExportFormat,
  LiveEntry,
  LiveState,
  LoopbackSource,
} from "../lib/types";

interface Props {
  config: Config;
  onChange: (next: Config) => void;
  entries: LiveEntry[];
  state: LiveState | null;
  sources: LoopbackSource[];
  /** Reason system audio is unavailable here, if it is. */
  unavailable: string | null;
  hasModel: boolean;
  onClear: () => void;
}

function clock(ms: number) {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

export function LivePanel({
  config,
  onChange,
  entries,
  state,
  sources,
  unavailable,
  hasModel,
  onClear,
}: Props) {
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  // Opening a system audio tap can take many seconds when Core Audio is still
  // tearing down a previous one, and the command does not return until it is
  // ready. Without this the button would simply look dead.
  const [starting, setStarting] = useState(false);
  const listRef = useRef<HTMLUListElement>(null);
  const listening = state?.listening ?? false;

  // Follow the transcript as it grows, but only while pinned to the bottom, so
  // scrolling back to read something is not yanked away.
  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const nearBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    if (nearBottom) el.scrollTop = el.scrollHeight;
  }, [entries]);

  async function toggle() {
    setError(null);
    setSaved(null);
    try {
      if (listening) {
        await api.stopListening();
      } else {
        setStarting(true);
        await api.startListening();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  }

  async function exportAs(format: ExportFormat) {
    setError(null);
    try {
      setSaved(await api.exportSession(format));
    } catch (e) {
      setError(String(e));
    }
  }

  if (unavailable) {
    return (
      <section className="panel">
        <div className="panel__head">
          <h2 className="panel__title">Listen</h2>
        </div>
        <p className="banner banner--warn">{unavailable}</p>
        <p className="settings__intro">
          Dictation still works. Only capturing your computer's own audio needs
          a newer system.
        </p>
      </section>
    );
  }

  return (
    <>
      <section className="panel live">
        <div className="panel__head">
          <h2 className="panel__title">Listen</h2>
          {listening && (
            <span className="live__clock">{clock(state?.elapsed_ms ?? 0)}</span>
          )}
        </div>

        <p className="settings__intro">
          Transcribes what your computer is playing — a call, a video — and your
          own microphone as a separate speaker. Nothing is typed into other
          apps while listening.
        </p>

        <label className="field">
          <span className="field__label">Audio source</span>
          <select
            className="select"
            disabled={listening}
            value={config.loopback_source ?? ""}
            onChange={(e) =>
              onChange({ ...config, loopback_source: e.target.value || null })
            }
          >
            <option value="">Automatic (system output)</option>
            {sources.map((s) => (
              <option key={s.name} value={s.name}>
                {s.name}
                {s.is_default ? " — default" : ""}
              </option>
            ))}
          </select>
        </label>

        <button
          className={`live__toggle ${listening ? "live__toggle--on" : ""}`}
          onClick={toggle}
          disabled={!hasModel || starting}
        >
          {starting
            ? "Starting…"
            : listening
              ? "Stop listening"
              : "Start listening"}
        </button>

        {starting && (
          <p className="banner banner--info">
            Opening the audio tap. This can take a few seconds.
          </p>
        )}

        {!hasModel && (
          <p className="banner banner--info">
            Download a voice model in Settings first.
          </p>
        )}

        {listening && !state?.receiving && (
          <p className="banner banner--info">
            Waiting for audio — nothing is playing yet.
          </p>
        )}

        {state?.lagging && (
          <p className="banner banner--warn">
            Transcription is running behind ({state.backlog} queued).
            {state.dropped > 0 && ` ${state.dropped} dropped.`} A smaller model
            keeps up better.
          </p>
        )}

        {error && <p className="banner banner--error">{error}</p>}
        {saved && (
          <p className="banner banner--info">Saved to {saved}</p>
        )}
      </section>

      <section className="panel transcript">
        <div className="panel__head">
          <h2 className="panel__title">Transcript</h2>
          {entries.length > 0 && (
            <div className="live__actions">
              <button
                className="ghost-btn"
                onClick={() => api.sessionText().then(writeText)}
              >
                Copy all
              </button>
              <button className="ghost-btn" onClick={() => exportAs("md")}>
                Export
              </button>
              <button className="ghost-btn" onClick={onClear}>
                Clear
              </button>
            </div>
          )}
        </div>

        {entries.length === 0 ? (
          <p className="transcript__empty">
            {listening
              ? "Listening… speech will appear here."
              : "Start listening to capture a conversation."}
          </p>
        ) : (
          <ul className="transcript__list live__list" ref={listRef}>
            {entries.map((e) => (
              <li key={e.id} className={`live__item live__item--${e.track}`}>
                <div className="live__meta">
                  <span className={`live__who live__who--${e.track}`}>
                    {e.track === "mic" ? "Me" : "Them"}
                  </span>
                  <time className="transcript__time">{clock(e.start_ms)}</time>
                </div>
                <p className="transcript__text">{e.text}</p>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
