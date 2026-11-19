import React from "react";
import { ArrowDownWideNarrow } from "lucide-react";
import { SortMethod } from "../types";

interface SortOptionsSectionProps {
  sortMethod: SortMethod;
  ratioW: number;
  ratioH: number;
  revRow: boolean;
  revSort: boolean;
  onSortMethodChange: (v: SortMethod) => void;
  onRatioWChange: (v: number) => void;
  onRatioHChange: (v: number) => void;
  onRevRowChange: (v: boolean) => void;
  onRevSortChange: (v: boolean) => void;
}

export const SortOptionsSection: React.FC<SortOptionsSectionProps> = ({
  sortMethod,
  ratioW,
  ratioH,
  revRow,
  revSort,
  onSortMethodChange,
  onRatioWChange,
  onRatioHChange,
  onRevRowChange,
  onRevSortChange,
}) => {
  return (
    <section className="section-card">
      <div className="section-title">
        <span className="section-title-left">
          <ArrowDownWideNarrow size={13} />
          Sort Collage Settings
        </span>
      </div>

      <div className="field-col">
        <span className="field-label">Sort Method</span>
        <select
          className="select"
          value={sortMethod}
          onChange={(e) => onSortMethodChange(e.target.value as SortMethod)}
        >
          <option value="bgr_sum">RGB Sum</option>
          <option value="av_hue">Average Hue</option>
          <option value="av_sat">Average Saturation</option>
          <option value="av_lum">Average Luminosity</option>
          <option value="rand">Random</option>
          <option value="none">None (Original Order)</option>
        </select>
      </div>

      <div className="field-row">
        <span className="field-label">Output Aspect Ratio</span>
        <div className="inline-pair" style={{ width: 116 }}>
          <input
            type="number"
            min={1}
            max={100}
            className="input"
            value={ratioW}
            onChange={(e) => onRatioWChange(Math.max(1, Number(e.target.value) || 1))}
          />
          <span className="inline-sep">:</span>
          <input
            type="number"
            min={1}
            max={100}
            className="input"
            value={ratioH}
            onChange={(e) => onRatioHChange(Math.max(1, Number(e.target.value) || 1))}
          />
        </div>
      </div>

      <div className="divider" />

      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={revRow}
          onChange={(e) => onRevRowChange(e.target.checked)}
        />
        <span>S-shaped row reversal</span>
      </label>

      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={revSort}
          onChange={(e) => onRevSortChange(e.target.checked)}
        />
        <span>Reverse sort order</span>
      </label>
    </section>
  );
};

