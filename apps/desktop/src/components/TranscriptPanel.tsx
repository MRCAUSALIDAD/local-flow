import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { TranscriptEntry } from "../lib/types";

interface Props {
  entries: TranscriptEntry[];
  onClear: () => void;
}

export function TranscriptPanel({ entries, onClear }: Props) {
  return (
    <section className="panel transcript">
      <div className="panel__head">
        <h2 className="panel__title">Transcripts</h2>
        {entries.length > 0 && (
          <button className="ghost-btn" onClick={onClear}>
            Clear
          </button>
        )}
      </div>

      {entries.length === 0 ? (
        <p className="transcript__empty">
          Nothing yet. Your dictations will appear here.
        </p>
      ) : (
        <ul className="transcript__list">
          {entries.map((e) => (
            <li key={e.id} className="transcript__item">
              <p className="transcript__text">{e.text}</p>
              <div className="transcript__row">
                <time className="transcript__time">
                  {new Date(e.at).toLocaleTimeString()}
                </time>
                <button
                  className="ghost-btn"
                  onClick={() => writeText(e.text)}
                >
                  Copy
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
