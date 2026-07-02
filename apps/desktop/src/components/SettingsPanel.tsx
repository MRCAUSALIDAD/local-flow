import {
  LANGUAGES,
  METRICS_CORNERS,
  type Config,
  type ModelInfo,
} from "../lib/types";

interface Props {
  config: Config;
  onChange: (next: Config) => void;
  error: string | null;
  microphones: string[];
  models: ModelInfo[];
  downloading: { id: string; pct: number } | null;
  onDownload: (id: string) => void;
}

export function SettingsPanel({
  config,
  onChange,
  error,
  microphones,
  models,
  downloading,
  onDownload,
}: Props) {
  return (
    <>
      <section className="panel settings">
        <div className="panel__head">
          <h2 className="panel__title">Voice model</h2>
        </div>
        <p className="settings__intro">
          Local Flow turns your voice into text on your own machine — nothing is
          sent to the cloud. Download one speech model to get started.
        </p>

        <div className="model-grid">
          {models.map((m) => {
            const busy = downloading?.id === m.id;
            return (
              <div
                key={m.id}
                className={`model-card ${m.active ? "model-card--active" : ""}`}
              >
                <div className="model-card__top">
                  <span className="model-card__label">{m.label}</span>
                  <span className="model-card__size">{m.size_mb} MB</span>
                </div>
                <span className="model-card__note">{m.note}</span>

                {busy ? (
                  <div className="progress">
                    <div
                      className="progress__bar"
                      style={{ width: `${downloading?.pct ?? 0}%` }}
                    />
                    <span className="progress__pct">{downloading?.pct ?? 0}%</span>
                  </div>
                ) : m.active ? (
                  <span className="model-card__badge">In use</span>
                ) : m.downloaded ? (
                  <button className="accent-btn" onClick={() => onDownload(m.id)}>
                    Use
                  </button>
                ) : (
                  <button
                    className="accent-btn"
                    disabled={!!downloading}
                    onClick={() => onDownload(m.id)}
                  >
                    Download
                  </button>
                )}
              </div>
            );
          })}
        </div>
        {error && <p className="settings__error">{error}</p>}
      </section>

      <section className="panel settings">
        <div className="panel__head">
          <h2 className="panel__title">Audio & Output</h2>
        </div>

        <label className="field">
          <span className="field__label">Microphone</span>
          <select
            className="select"
            value={config.input_device ?? ""}
            onChange={(e) =>
              onChange({ ...config, input_device: e.target.value || null })
            }
          >
            <option value="">Automatic (system default)</option>
            {microphones.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field__label">Language</span>
          <select
            className="select"
            value={config.language}
            onChange={(e) => onChange({ ...config, language: e.target.value })}
          >
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.label}
              </option>
            ))}
          </select>
        </label>

        <Toggle
          label="Type into active app"
          hint="Write the transcript wherever your cursor is"
          checked={config.auto_type}
          onChange={(v) => onChange({ ...config, auto_type: v })}
        />

        <Toggle
          label="Copy to clipboard"
          hint="Also place the transcript on the clipboard"
          checked={config.copy_to_clipboard}
          onChange={(v) => onChange({ ...config, copy_to_clipboard: v })}
        />

        <Toggle
          label="Métricas fijas en pantalla"
          hint="Panel siempre visible con CPU, RAM y tiempos"
          checked={config.show_metrics}
          onChange={(v) => onChange({ ...config, show_metrics: v })}
        />

        {config.show_metrics && (
          <label className="field">
            <span className="field__label">Esquina de las métricas</span>
            <select
              className="select"
              value={config.metrics_corner}
              onChange={(e) =>
                onChange({ ...config, metrics_corner: e.target.value })
              }
            >
              {METRICS_CORNERS.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </select>
          </label>
        )}

        <div className="field">
          <span className="field__label">Hotkey</span>
          <span className="kbd-row">
            <kbd>⌥</kbd>
            <kbd>Space</kbd>
            <span className="field__hint">Hold to talk, release to transcribe</span>
          </span>
        </div>
      </section>
    </>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="toggle" onClick={() => onChange(!checked)}>
      <div className="toggle__text">
        <span className="field__label">{label}</span>
        <span className="field__hint">{hint}</span>
      </div>
      <span className={`switch ${checked ? "switch--on" : ""}`}>
        <span className="switch__knob" />
      </span>
    </div>
  );
}
