import { getCurrentWindow } from "@tauri-apps/api/window";

export function TitleBar() {
  const win = getCurrentWindow();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar__brand" data-tauri-drag-region>
        <span className="titlebar__logo" />
        <span className="titlebar__name">LOCAL&nbsp;FLOW</span>
      </div>
      <div className="titlebar__controls">
        <button
          className="winbtn"
          aria-label="Minimize"
          onClick={() => win.minimize()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="1" y1="5" x2="9" y2="5" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        </button>
        <button
          className="winbtn winbtn--close"
          aria-label="Close"
          onClick={() => win.hide()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="1.5" y1="1.5" x2="8.5" y2="8.5" stroke="currentColor" strokeWidth="1.4" />
            <line x1="8.5" y1="1.5" x2="1.5" y2="8.5" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        </button>
      </div>
    </header>
  );
}
