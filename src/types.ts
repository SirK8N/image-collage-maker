export type ResizeOption = "center" | "stretch" | "fit";
export type SortMethod =
  | "none"
  | "bgr_sum"
  | "av_hue"
  | "av_sat"
  | "av_lum"
  | "rand";
export type ColorSpace = "lab" | "luv" | "hsv" | "hsl" | "bgr";
export type DistanceMetric =
  | "euclidean"
  | "cityblock"
  | "chebyshev"
  | "cosine";
export type BlendMode = "brightness" | "alpha";
export type FairnessMode = "fair" | "unfair";
export type AppMode = "mosaic" | "sort";

export interface LoadTilesRequest {
  folderPath: string;
  tileWidth: number;
  tileHeight: number | null;
  inferHeight: boolean;
  resizeOpt: ResizeOption;
  autoRotate: number;
  recursive: boolean;
}

export interface LoadTilesResponse {
  tileCount: number;
  skippedCount: number;
  tileWidth: number;
  tileHeight: number;
  gridWidth: number;
  gridHeight: number;
  outputWidth: number;
  outputHeight: number;
  previewDataUrl: string;
  elapsedMs: number;
}

export interface LoadDestinationResponse {
  path: string;
  width: number;
  height: number;
  hasAlpha: boolean;
  previewDataUrl: string;
}

export interface GenerateSortRequest {
  sortMethod: SortMethod;
  ratioW: number;
  ratioH: number;
  revRow: boolean;
  revSort: boolean;
}

export interface GenerateMosaicRequest {
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
}

export interface GenerationResponse {
  gridWidth: number;
  gridHeight: number;
  outputWidth: number;
  outputHeight: number;
  previewDataUrl: string;
  hasTileInfo: boolean;
  elapsedMs: number;
}

export interface PreviewResponse {
  previewDataUrl: string;
  width: number;
  height: number;
}

export interface ProgressEvent {
  stage: string;
  current: number;
  total: number;
  message: string;
}

export interface LogEvent {
  level: "info" | "warn" | "error" | "success" | string;
  message: string;
}

export interface LogEntry {
  id: number;
  time: string;
  level: string;
  message: string;
}

export interface SamplePaths {
  tilesDir: string;
  destImg: string;
}

