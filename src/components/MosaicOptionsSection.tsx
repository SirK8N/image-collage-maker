import React from "react";
import { ImagePlus, Sliders, Sparkles } from "lucide-react";
import {
  BlendMode,
  ColorSpace,
  DistanceMetric,
  FairnessMode,
} from "../types";

interface MosaicOptionsSectionProps {
  destPath: string;
  destDims: { width: number; height: number; hasAlpha: boolean } | null;
  colorspace: ColorSpace;
  metric: DistanceMetric;
  fairness: FairnessMode;
  dup: number;
  maxWidth: number;
  freqMul: number;
  dither: boolean;
  deterministic: boolean;
  salient: boolean;
  lowerThresh: number;
  transparent: boolean;
  blendMode: BlendMode;
  blendLevel: number;
  busy: boolean;
  onPickDest: () => void;
  onColorspaceChange: (v: ColorSpace) => void;
  onMetricChange: (v: DistanceMetric) => void;
  onFairnessChange: (v: FairnessMode) => void;
  onDupChange: (v: number) => void;
  onMaxWidthChange: (v: number) => void;
  onFreqMulChange: (v: number) => void;
  onDitherChange: (v: boolean) => void;
  onDeterministicChange: (v: boolean) => void;
  onSalientChange: (v: boolean) => void;
  onLowerThreshChange: (v: number) => void;
  onTransparentChange: (v: boolean) => void;
  onBlendModeChange: (v: BlendMode) => void;
  onBlendLevelChange: (v: number) => void;
}

export const MosaicOptionsSection: React.FC<MosaicOptionsSectionProps> = ({
  destPath,
  destDims,
  colorspace,
  metric,
  fairness,
  dup,
  maxWidth,
  freqMul,
  dither,
  deterministic,
  salient,
  lowerThresh,
  transparent,
  blendMode,
  blendLevel,
  busy,
  onPickDest,
  onColorspaceChange,
  onMetricChange,
  onFairnessChange,
  onDupChange,
  onMaxWidthChange,
  onFreqMulChange,
  onDitherChange,
  onDeterministicChange,
  onSalientChange,
  onLowerThreshChange,
  onTransparentChange,
  onBlendModeChange,
  onBlendLevelChange,
}) => {
  const isUnfair = fairness === "unfair";
  const ditherActive = isUnfair && dither;

  return (
    <>
      <section className="section-card">
        <div className="section-title">
          <span className="section-title-left">
            <ImagePlus size={13} />
            Target Image
          </span>
          {destDims && (
            <span className="pill">
              {destDims.width}×{destDims.height}
              {destDims.hasAlpha ? " α" : ""}
            </span>
          )}
        </div>

        <div className="path-box" title={destPath || "No target image selected"}>
          {destPath || "No destination image selected"}
        </div>

        <button className="btn" onClick={onPickDest} disabled={busy}>
          <ImagePlus size={14} />
          Choose Target Image
        </button>
      </section>

      <section className="section-card">
        <div className="section-title">
          <span className="section-title-left">
            <Sparkles size={13} />
            Assignment &amp; Matching
          </span>
        </div>

        <div className="segmented">
          <button
            className={`segmented-btn ${fairness === "fair" ? "active" : ""}`}
            onClick={() => onFairnessChange("fair")}
            title="Every tile is used the same number of times (Jonker-Volgenant LAP)"
          >
            Fair (LAP)
          </button>
          <button
            className={`segmented-btn ${fairness === "unfair" ? "active" : ""}`}
            onClick={() => onFairnessChange("unfair")}
            title="Allow repeating best-fitting tiles freely or with a fairness penalty"
          >
            Unfair (Best Fit)
          </button>
        </div>

        {fairness === "fair" ? (
          <div className="field-row">
            <span
              className="field-label"
              title="How many times to duplicate the tile set (fractions like 0.5 or 2.5 are supported)"
            >
              Duplicates
            </span>
            <input
              type="number"
              min={0.1}
              step={0.5}
              className="input"
              style={{ width: 92, textAlign: "center" }}
              value={dup}
              onChange={(e) => {
                const v = parseFloat(e.target.value);
                if (!Number.isNaN(v)) onDupChange(v);
              }}
            />
          </div>
        ) : (
          <>
            <div className="field-grid-2">
              <div className="field-col">
                <span className="field-label" title="Grid width in number of tiles">
                  Max Width
                </span>
                <input
                  type="number"
                  min={2}
                  max={500}
                  className="input"
                  value={maxWidth}
                  onChange={(e) =>
                    onMaxWidthChange(Math.max(2, Number(e.target.value) || 2))
                  }
                />
              </div>
              <div className="field-col">
                <span
                  className="field-label"
                  title="Penalty for reusing tiles. 0 = pure best fit, higher = fairer usage"
                >
                  Freq Multiplier
                </span>
                <input
                  type="number"
                  min={0}
                  step={0.5}
                  className="input"
                  value={freqMul}
                  onChange={(e) => {
                    const v = parseFloat(e.target.value);
                    if (!Number.isNaN(v) && v >= 0) onFreqMulChange(v);
                  }}
                />
              </div>
            </div>

            <div className="field-grid-2">
              <label
                className={`checkbox-row ${transparent ? "disabled" : ""}`}
                title="Floyd-Steinberg color error diffusion across neighboring tiles"
              >
                <input
                  type="checkbox"
                  checked={dither}
                  disabled={transparent}
                  onChange={(e) => onDitherChange(e.target.checked)}
                />
                <span>Dithering</span>
              </label>

              <label
                className={`checkbox-row ${ditherActive ? "disabled" : ""}`}
                title="Process grid blocks in deterministic scanline order"
              >
                <input
                  type="checkbox"
                  checked={ditherActive ? true : deterministic}
                  disabled={ditherActive}
                  onChange={(e) => onDeterministicChange(e.target.checked)}
                />
                <span>Deterministic</span>
              </label>
            </div>
          </>
        )}

        <div className="divider" />

        <div className="field-grid-2">
          <div className="field-col">
            <span className="field-label">Colorspace</span>
            <select
              className="select"
              value={colorspace}
              onChange={(e) => onColorspaceChange(e.target.value as ColorSpace)}
            >
              <option value="lab">CIE LAB</option>
              <option value="luv">CIE LUV</option>
              <option value="hsv">HSV</option>
              <option value="hsl">HSL</option>
              <option value="bgr">RGB / BGR</option>
            </select>
          </div>
          <div className="field-col">
            <span className="field-label">Distance Metric</span>
            <select
              className="select"
              value={metric}
              onChange={(e) => onMetricChange(e.target.value as DistanceMetric)}
            >
              <option value="euclidean">Euclidean</option>
              <option value="cityblock">Cityblock (L1)</option>
              <option value="chebyshev">Chebyshev (L∞)</option>
              <option value="cosine">Cosine</option>
            </select>
          </div>
        </div>
      </section>

      <section className="section-card">
        <div className="section-title">
          <span className="section-title-left">
            <Sliders size={13} />
            Masking &amp; Color Blend
          </span>
        </div>

        <label
          className={`checkbox-row ${transparent || ditherActive ? "disabled" : ""}`}
          title="Keep only foreground salient regions detected from the target image"
        >
          <input
            type="checkbox"
            checked={salient}
            disabled={transparent || ditherActive}
            onChange={(e) => onSalientChange(e.target.checked)}
          />
          <span>Salient objects only</span>
        </label>

        {salient && !transparent && !ditherActive && (
          <div className="slider-wrap">
            <div className="slider-header">
              <span>Saliency Threshold</span>
              <span style={{ fontFamily: "var(--font-mono)" }}>
                {Math.round(lowerThresh * 100)}%
              </span>
            </div>
            <input
              type="range"
              min={1}
              max={99}
              className="slider"
              value={Math.round(lowerThresh * 100)}
              onChange={(e) => onLowerThreshChange(Number(e.target.value) / 100)}
            />
          </div>
        )}

        <label
          className={`checkbox-row ${ditherActive ? "disabled" : ""}`}
          title="Preserve transparent background of a PNG destination image"
        >
          <input
            type="checkbox"
            checked={transparent}
            disabled={ditherActive}
            onChange={(e) => onTransparentChange(e.target.checked)}
          />
          <span>Transparency masking</span>
        </label>

        <div className="divider" />

        <div className="segmented">
          <button
            className={`segmented-btn ${blendMode === "brightness" ? "active" : ""}`}
            onClick={() => onBlendModeChange("brightness")}
            title="Blend only the HSL lightness channel from the target image"
          >
            Brightness Blend
          </button>
          <button
            className={`segmented-btn ${blendMode === "alpha" ? "active" : ""}`}
            onClick={() => onBlendModeChange("alpha")}
            title="Standard alpha overlay with the target image"
          >
            Alpha Blend
          </button>
        </div>

        <div className="slider-wrap">
          <div className="slider-header">
            <span>Blend Strength</span>
            <span style={{ fontFamily: "var(--font-mono)" }}>
              {Math.round(blendLevel * 100)}%
            </span>
          </div>
          <input
            type="range"
            min={0}
            max={100}
            className="slider"
            value={Math.round(blendLevel * 100)}
            onChange={(e) => onBlendLevelChange(Number(e.target.value) / 100)}
          />
        </div>
      </section>
    </>
  );
};

