import React, { useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronUp, Terminal, Trash2 } from "lucide-react";
import { LogEntry } from "../types";

interface LogDrawerProps {
  logs: LogEntry[];
  onClear: () => void;
}

export const LogDrawer: React.FC<LogDrawerProps> = ({ logs, onClear }) => {
  const [open, setOpen] = useState(true);
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open && bodyRef.current) {
      bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
    }
  }, [logs, open]);

  const latest = logs.length > 0 ? logs[logs.length - 1] : null;

  return (
    <div className="log-drawer">
      <div className="log-header" onClick={() => setOpen((v) => !v)}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
          <Terminal size={13} />
          <span>Activity Log ({logs.length})</span>
          {!open && latest && (
            <span
              className={`log-${latest.level}`}
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              — {latest.message}
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          {logs.length > 0 && (
            <button
              className="btn-icon"
              title="Clear logs"
              onClick={(e) => {
                e.stopPropagation();
                onClear();
              }}
            >
              <Trash2 size={13} />
            </button>
          )}
          {open ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
        </div>
      </div>

      {open && (
        <div className="log-body" ref={bodyRef}>
          {logs.length === 0 ? (
            <div style={{ color: "var(--text-muted)" }}>
              Ready. Load a folder of tiles to begin.
            </div>
          ) : (
            logs.map((entry) => (
              <div key={entry.id} className="log-line">
                <span className="log-time">{entry.time}</span>
                <span className={`log-${entry.level}`}>{entry.message}</span>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
};

