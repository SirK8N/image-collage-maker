use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct TileImage {
    pub width: u32,
    pub height: u32,
    /// Contiguous RGB bytes: width * height * 3
    pub rgb: Vec<u8>,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResizeOption {
    Center,
    Stretch,
    Fit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortMethod {
    None,
    BgrSum,
    AvHue,
    AvSat,
    AvLum,
    Rand,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorSpace {
    Lab,
    Luv,
    Hsv,
    Hsl,
    Bgr,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    Euclidean,
    Cityblock,
    Chebyshev,
    Cosine,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Brightness,
    Alpha,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FairnessMode {
    Fair,
    Unfair,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadTilesRequest {
    pub folder_path: String,
    pub tile_width: u32,
    pub tile_height: Option<u32>,
    pub infer_height: bool,
    pub resize_opt: ResizeOption,
    pub auto_rotate: i32,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadTilesResponse {
    pub tile_count: usize,
    pub skipped_count: usize,
    pub tile_width: u32,
    pub tile_height: u32,
    pub grid_width: u32,
    pub grid_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub preview_data_url: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadDestinationResponse {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub preview_data_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSortRequest {
    pub sort_method: SortMethod,
    pub ratio_w: u32,
    pub ratio_h: u32,
    pub rev_row: bool,
    pub rev_sort: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMosaicRequest {
    pub colorspace: ColorSpace,
    pub metric: DistanceMetric,
    pub fairness: FairnessMode,
    /// Fair mode duplication factor (can be fractional > 0)
    pub dup: f64,
    /// Unfair mode grid width
    pub max_width: u32,
    /// Unfair mode frequency penalty multiplier
    pub freq_mul: f32,
    pub dither: bool,
    pub deterministic: bool,
    pub salient: bool,
    /// Saliency threshold in [0.01, 0.99]
    pub lower_thresh: f32,
    pub transparent: bool,
    pub blend_mode: BlendMode,
    /// Blend level in [0.0, 1.0]
    pub blend_level: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationResponse {
    pub grid_width: u32,
    pub grid_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub preview_data_url: String,
    pub has_tile_info: bool,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResponse {
    pub preview_data_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub stage: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub level: String,
    pub message: String,
}

