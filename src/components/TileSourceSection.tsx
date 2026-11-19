import React from "react";
import { FolderOpen, Layers, RefreshCw } from "lucide-react";
import { ResizeOption } from "../types";

interface TileSourceSectionProps {
  folderPath: string;
  tileCount: number;
  tileWidth: number;
  tileHeight: number;
  inferHeight: boolean;
  resizeOpt: ResizeOption;
  autoRotate: number;
  recursive: boolean;
  busy: boolean;
  onTileWidthChange: (v: number) => void;
  onTileHeightChange: (v: number) => void;
  onInferHeightChange: (v: boolean) => void;
  onResizeOptChange: (v: ResizeOption) => void;
  onAutoRotateChange: (v: number) => void;
  onRecursiveChange: (v: boolean) => void;
  onPickFolder: () => void;
  onReload: () => void;
}

export const TileSourceSection: React.FC<TileSourceSectionProps> = ({
  folderPath,
  tileCount,
  tileWidth,
  tileHeight,
  inferHeight,
  resizeOpt,
  autoRotate,
  recursive,
  busy,
  onTileWidthChange,
  onTileHeightChange,
  onInferHeightChange,
  onResizeOptChange,
  onAutoRotateChange,
  onRecursiveChange,
  onPickFolder,
  onReload,
}) => {
  return (
    <section className="section-card">
      <div className="section-title">
        <span className="section-title-left">
          <Layers size={13} />
          Tile Library
        </span>
        {tileCount > 0 && (
          <span className="pill pill-accent">{tileCount} tiles</span>
        )}
      </div>

      <div className="path-box" title={folderPath || "No folder selected"}>
        {folderPath || "No tile folder selected"}
      </div>

      <div className="field-row">
        <span className="field-label" title="Width & height of each tile in pixels">
          Tile Size (px)
        </span>
        <div className="inline-pair" style={{ width: 136 }}>
          <input
            type="number"
            min={2}
            max={512}
            className="input"
            value={tileWidth}
            onChange={(e) => onTileWidthChange(Math.max(2, Number(e.target.value) || 2))}
          />
          <span className="inline-sep">×</span>
          <input
            type="number"
            min={2}
            max={512}
            className="input"
            disabled={inferHeight}
            value={tileHeight}
            onChange={(e) => onTileHeightChange(Math.max(2, Number(e.target.value) || 2))}
          />
        </div>
      </div>

      <div className="field-grid-2">
        <label
          className="checkbox-row"
          title="Infer height from width and most common tile aspect ratio"
        >
          <input
            type="checkbox"
            checked={inferHeight}
            onChange={(e) => onInferHeightChange(e.target.checked)}
          />
          <span>Infer height</span>
        </label>
        <label
          className="checkbox-row"
          title="Recursively scan sub-folders for images"
        >
          <input
            type="checkbox"
            checked={recursive}
            onChange={(e) => onRecursiveChange(e.target.checked)}
          />
          <span>Sub-folders</span>
        </label>
      </div>

      <div className="field-grid-2">
        <div className="field-col">
          <span className="field-label">Resize</span>
          <select
            className="select"
            value={resizeOpt}
            onChange={(e) => onResizeOptChange(e.target.value as ResizeOption)}
          >
            <option value="center">Center Crop</option>
            <option value="stretch">Stretch</option>
            <option value="fit">Fit (Pad White)</option>
          </select>
        </div>

        <div className="field-col">
          <span className="field-label">Auto-Rotate</span>
          <select
            className="select"
            value={autoRotate}
            onChange={(e) => onAutoRotateChange(Number(e.target.value))}
          >
            <option value={0}>Off</option>
            <option value={1}>90° CCW</option>
            <option value={-1}>90° CW</option>
          </select>
        </div>
      </div>

      <div className="btn-row">
        <button className="btn" onClick={onPickFolder} disabled={busy}>
          <FolderOpen size={14} />
          Load Tiles Folder
        </button>
        <button
          className="btn"
          title="Reload tiles with updated tile settings"
          onClick={onReload}
          disabled={busy || !folderPath}
        >
          <RefreshCw size={14} />
        </button>
      </div>
    </section>
  );
};

