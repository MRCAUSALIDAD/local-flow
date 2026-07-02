import type { FlowStatus } from "../lib/types";

const LABELS: Record<FlowStatus, string> = {
  idle: "Ready",
  listening: "Listening…",
  transcribing: "Transcribing…",
};

const HINTS: Record<FlowStatus, string> = {
  idle: "Hold ⌥ Space anywhere — it types into your active app",
  listening: "Release to transcribe",
  transcribing: "Running whisper locally",
};

interface Props {
  status: FlowStatus;
  disabled: boolean;
  onPressStart: () => void;
  onPressEnd: () => void;
}

export function StatusOrb({ status, disabled, onPressStart, onPressEnd }: Props) {
  return (
    <section className="orb-stage">
      <button
        className={`orb orb--${status}`}
        data-disabled={disabled}
        onPointerDown={(e) => {
          if (disabled) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          onPressStart();
        }}
        onPointerUp={() => !disabled && onPressEnd()}
        onPointerCancel={() => !disabled && onPressEnd()}
        aria-label="Push to talk"
      >
        <span className="orb__ring orb__ring--1" />
        <span className="orb__ring orb__ring--2" />
        <span className="orb__core">
          <MicIcon />
        </span>
        <span className="orb__wave" />
      </button>
      <div className="orb-meta">
        <span className="orb-meta__label">{LABELS[status]}</span>
        <span className="orb-meta__hint">
          {disabled ? "Load a model in Settings to start" : HINTS[status]}
        </span>
      </div>
    </section>
  );
}

function MicIcon() {
  return (
    <svg width="34" height="34" viewBox="0 0 24 24" fill="none">
      <rect x="9" y="2" width="6" height="12" rx="3" fill="currentColor" />
      <path
        d="M5 11a7 7 0 0 0 14 0M12 18v4M8 22h8"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
