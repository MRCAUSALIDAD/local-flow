import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Config, FlowStatus } from "./lib/types";
import "./Overlay.css";

type Sys = { cpuPct: number; ramMb: number };
type Tx = { audioSecs: number; transcribeMs: number; rtf: number; speed: number };

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div className="metrics__row">
      <span>{k}</span>
      <span>{v}</span>
    </div>
  );
}

export function Overlay() {
  const [status, setStatus] = useState<FlowStatus>("idle");
  const [sys, setSys] = useState<Sys | null>(null);
  const [tx, setTx] = useState<Tx | null>(null);
  const [hover, setHover] = useState(false);
  const [pinned, setPinned] = useState(false);

  useEffect(() => {
    invoke<Config>("get_config")
      .then((c) => setPinned(c.show_metrics))
      .catch(() => {});

    const subs = Promise.all([
      listen<FlowStatus>("flow-status", (e) => setStatus(e.payload)),
      listen<Sys>("flow-metrics", (e) => setSys(e.payload)),
      listen<Tx>("flow-metrics-transcribe", (e) => setTx(e.payload)),
      listen<Config>("flow-config", (e) => setPinned(e.payload.show_metrics)),
    ]);
    return () => {
      subs.then((fns) => fns.forEach((f) => f()));
    };
  }, []);

  function enter() {
    if (pinned) return;
    setHover(true);
    invoke("overlay_expand").catch(() => {});
  }

  function leave() {
    if (pinned) return;
    setHover(false);
    invoke("overlay_collapse").catch(() => {});
  }

  const showPanel = pinned || hover;

  return (
    <div
      className={`wrap ${pinned ? "wrap--pinned" : ""}`}
      onMouseEnter={enter}
      onMouseLeave={leave}
    >
      {showPanel && (
        <div className="metrics">
          <div className="metrics__title">Local Flow · métricas</div>
          <Row k="CPU" v={sys ? `${sys.cpuPct.toFixed(0)} %` : "—"} />
          <Row k="RAM" v={sys ? `${sys.ramMb.toFixed(0)} MB` : "—"} />
          <Row k="Audio" v={tx ? `${tx.audioSecs.toFixed(1)} s` : "—"} />
          <Row k="Transcripción" v={tx ? `${tx.transcribeMs.toFixed(0)} ms` : "—"} />
          <Row k="RTF" v={tx ? tx.rtf.toFixed(2) : "—"} />
          <Row k="Velocidad" v={tx ? `${tx.speed.toFixed(1)}×` : "—"} />
        </div>
      )}
      <span className={`dot dot--${status}`} />
    </div>
  );
}
