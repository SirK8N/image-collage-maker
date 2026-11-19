import React, { useEffect, useRef, useState } from "react";
import {
  Eye,
  Image as ImageIcon,
  Maximize2,
  RotateCcw,
  Sparkles,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { ProgressEvent } from "../types";

interface PreviewCanvasProps {
  imageUrl: string | null;
  compareUrl?: string | null;
  busy: boolean;
  progress: ProgressEvent | null;
  hasSamples: boolean;
  onLoadSamples: () => void;
}

export const PreviewCanvas: React.FC<PreviewCanvasProps> = ({
  imageUrl,
  compareUrl,
  busy,
  progress,
  hasSamples,
  onLoadSamples,
}) => {
  const stageRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [comparing, setComparing] = useState(false);
  const dragStart = useRef({ x: 0, y: 0, ox: 0, oy: 0 });
  const displayedUrl = comparing && compareUrl ? compareUrl : imageUrl;

  const fitToStage = () => {
    const stage = stageRef.current;
    const img = imgRef.current;
    if (!stage || !img || !img.naturalWidth || !img.naturalHeight) return;
    const pad = 40;
    const availW = Math.max(100, stage.clientWidth - pad);
    const availH = Math.max(100, stage.clientHeight - pad);
    const fit = Math.min(availW / img.naturalWidth, availH / img.naturalHeight, 1.5);
    setScale(Math.max(0.05, Number(fit.toFixed(3))));
    setOffset({ x: 0, y: 0 });
  };

  useEffect(() => {
    if (!imageUrl) {
      setScale(1);
      setOffset({ x: 0, y: 0 });
    }
  }, [imageUrl]);

  const handleWheel = (e: React.WheelEvent) => {
    if (!imageUrl) return;
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
    setScale((prev) => Math.min(8, Math.max(0.05, prev * factor)));
  };

  const handlePointerDown = (e: React.PointerEvent) => {
    if (!imageUrl || e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setDragging(true);
    dragStart.current = { x: e.clientX, y: e.clientY, ox: offset.x, oy: offset.y };
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!dragging) return;
    setOffset({
      x: dragStart.current.ox + (e.clientX - dragStart.current.x),
      y: dragStart.current.oy + (e.clientY - dragStart.current.y),
    });
  };

  const handlePointerUp = () => {
    setDragging(false);
  };

  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.current / progress.total) * 100))
      : 0;

  return (
    <div
      ref={stageRef}
      className={`canvas-stage ${imageUrl ? (dragging ? "panning" : "pannable") : ""}`}
      onWheel={handleWheel}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
    >
      {imageUrl ? (
        <div
          className="canvas-image-wrap"
          style={{
            transform: `translate3d(${offset.x}px, ${offset.y}px, 0) scale(${scale})`,
          }}
        >
          <img
            ref={imgRef}
            src={displayedUrl ?? undefined}
            alt="Photomosaic preview"
            className="canvas-image"
            onLoad={() => {
              if (!comparing) fitToStage();
            }}
            draggable={false}
          />
        </div>
      ) : (
        <div className="canvas-empty">
          <div className="canvas-empty-icon">
            <ImageIcon size={24} />
          </div>
          <h2>Create a Photomosaic or Collage</h2>
          <p>
            Load a folder of tile images from the inspector on the right, choose a
            target image (or sort mode), and generate high-resolution artwork powered
            by Rust.
          </p>
          {hasSamples && (
            <button
              className="btn btn-primary"
              onClick={onLoadSamples}
              disabled={busy}
            >
              <Sparkles size={15} />
              Try Bundled Examples
            </button>
          )}
        </div>
      )}

      {busy && progress && (
        <div className="progress-overlay">
          <div className="progress-header">
            <span>{progress.message}</span>
            <span style={{ fontFamily: "var(--font-mono)", color: "var(--cyan)" }}>
              {pct}%
            </span>
          </div>
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${Math.max(4, pct)}%` }} />
          </div>
        </div>
      )}

      {imageUrl && (
        <div
          className="canvas-controls"
          onPointerDown={(e) => e.stopPropagation()}
        >
          {compareUrl && (
            <button
              className="btn-icon"
              title="Hold to compare with target image"
              style={
                comparing
                  ? { background: "var(--accent)", color: "#fff" }
                  : undefined
              }
              onPointerDown={() => setComparing(true)}
              onPointerUp={() => setComparing(false)}
              onPointerLeave={() => setComparing(false)}
            >
              <Eye size={15} />
            </button>
          )}
          <button
            className="btn-icon"
            title="Zoom out"
            onClick={() => setScale((s) => Math.max(0.05, s / 1.2))}
          >
            <ZoomOut size={15} />
          </button>
          <span className="zoom-label">{Math.round(scale * 100)}%</span>
          <button
            className="btn-icon"
            title="Zoom in"
            onClick={() => setScale((s) => Math.min(8, s * 1.2))}
          >
            <ZoomIn size={15} />
          </button>
          <button
            className="btn-icon"
            title="Actual size (100%)"
            onClick={() => {
              setScale(1);
              setOffset({ x: 0, y: 0 });
            }}
          >
            <Maximize2 size={14} />
          </button>
          <button
            className="btn-icon"
            title="Fit to canvas"
            onClick={fitToStage}
          >
            <RotateCcw size={14} />
          </button>
        </div>
      )}
    </div>
  );
};
