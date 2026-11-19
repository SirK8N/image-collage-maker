use crate::mosaic::{
    blend_images, calc_grid_size, encode_preview_data_url, generate_photomosaic,
    generate_sorted_collage, make_collage_grid, render_saliency_preview,
};
use crate::saliency::compute_fine_grained_saliency;
use crate::tile_loader::load_tiles_from_dir;
use crate::types::{
    BlendMode, GenerateMosaicRequest, GenerateSortRequest, GenerationResponse,
    LoadDestinationResponse, LoadTilesRequest, LoadTilesResponse, LogEvent, PreviewResponse,
    ProgressEvent, TileImage,
};
use image::{DynamicImage, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

const PREVIEW_MAX_DIM: u32 = 1800;

#[derive(Default)]
pub struct AppStateInner {
    pub tiles: Vec<TileImage>,
    pub dest_img: Option<RgbaImage>,
    pub dest_has_alpha: bool,
    pub dest_saliency: Option<Vec<f32>>,
    pub raw_mosaic: Option<RgbaImage>,
    pub current_result: Option<RgbaImage>,
    pub current_tile_info: Option<String>,
}

pub struct AppState(pub Arc<Mutex<AppStateInner>>);

impl Default for AppState {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(AppStateInner::default())))
    }
}

fn emit_log(app: &AppHandle, level: &str, message: impl Into<String>) {
    let _ = app.emit(
        "mosaic-log",
        LogEvent {
            level: level.to_string(),
            message: message.into(),
        },
    );
}

fn emit_progress(
    app: &AppHandle,
    stage: &str,
    current: usize,
    total: usize,
    message: impl Into<String>,
) {
    let _ = app.emit(
        "mosaic-progress",
        ProgressEvent {
            stage: stage.to_string(),
            current,
            total,
            message: message.into(),
        },
    );
}

#[tauri::command]
pub async fn load_tiles(
    app: AppHandle,
    state: State<'_, AppState>,
    req: LoadTilesRequest,
) -> Result<LoadTilesResponse, String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let start = Instant::now();
        let dir = PathBuf::from(&req.folder_path);
        emit_log(&app, "info", format!("Scanning tiles in {}...", req.folder_path));
        emit_progress(&app, "load_tiles", 0, 1, "Scanning folder...".to_string());

        let app_for_prog = app.clone();
        let loaded = load_tiles_from_dir(
            &dir,
            req.tile_width,
            req.tile_height,
            req.infer_height,
            req.resize_opt,
            req.auto_rotate,
            req.recursive,
            move |cur, tot| {
                emit_progress(
                    &app_for_prog,
                    "load_tiles",
                    cur,
                    tot,
                    format!("Decoding tiles ({cur}/{tot})..."),
                );
            },
        )?;

        emit_log(
            &app,
            "info",
            format!(
                "Loaded {} tiles ({}x{} px). Skipped {} non-image files.",
                loaded.tiles.len(),
                loaded.tile_width,
                loaded.tile_height,
                loaded.skipped
            ),
        );

        let grid = calc_grid_size(
            16,
            10,
            loaded.tiles.len(),
            loaded.tile_width,
            loaded.tile_height,
        );
        let tile_refs: Vec<&TileImage> = loaded.tiles.iter().collect();
        let overview = make_collage_grid(grid, &tile_refs, false);
        let preview_data_url = encode_preview_data_url(&overview.image, PREVIEW_MAX_DIM)?;

        let out_w = overview.image.width();
        let out_h = overview.image.height();
        let tile_count = loaded.tiles.len();

        {
            let mut guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard.tiles = loaded.tiles;
            guard.raw_mosaic = None;
            guard.current_result = Some(overview.image);
            guard.current_tile_info = Some(overview.tile_info);
        }

        let elapsed_ms = start.elapsed().as_millis();
        emit_progress(&app, "done", 1, 1, "Tiles loaded".to_string());
        emit_log(&app, "success", format!("Tiles ready in {:.2}s.", elapsed_ms as f64 / 1000.0));

        Ok(LoadTilesResponse {
            tile_count,
            skipped_count: loaded.skipped,
            tile_width: loaded.tile_width,
            tile_height: loaded.tile_height,
            grid_width: grid.0,
            grid_height: grid.1,
            output_width: out_w,
            output_height: out_h,
            preview_data_url,
            elapsed_ms,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn load_destination(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<LoadDestinationResponse, String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        if !p.is_file() {
            return Err(format!("Destination file not found: {path}"));
        }

        let dyn_img = image::open(p).map_err(|e| format!("Failed to open image: {e}"))?;
        let has_alpha = dyn_img.color().has_alpha();
        let rgba = dyn_img.to_rgba8();
        let (width, height) = rgba.dimensions();

        // Check if alpha channel actually contains any transparent pixels
        let actually_transparent = has_alpha && rgba.pixels().any(|px| px[3] < 255);
        let preview_data_url = encode_preview_data_url(&rgba, PREVIEW_MAX_DIM)?;

        {
            let mut guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard.dest_img = Some(rgba.clone());
            guard.dest_has_alpha = actually_transparent;
            guard.dest_saliency = None;
            guard.raw_mosaic = None;
            guard.current_result = Some(rgba);
            guard.current_tile_info = None;
        }

        emit_log(
            &app,
            "info",
            format!(
                "Destination loaded: {width}x{height} px{}",
                if actually_transparent {
                    " (transparent background detected)"
                } else {
                    ""
                }
            ),
        );

        Ok(LoadDestinationResponse {
            path,
            width,
            height,
            has_alpha: actually_transparent,
            preview_data_url,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn preview_saliency(
    state: State<'_, AppState>,
    threshold: f32,
) -> Result<PreviewResponse, String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (dest, maybe_sal) = {
            let guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            let dest = guard
                .dest_img
                .clone()
                .ok_or_else(|| "No destination image loaded.".to_string())?;
            (dest, guard.dest_saliency.clone())
        };

        let (w, h) = dest.dimensions();
        let sal_map = match maybe_sal {
            Some(m) => m,
            None => {
                let mut rgb = Vec::with_capacity((w * h * 3) as usize);
                for px in dest.pixels() {
                    rgb.push(px[0]);
                    rgb.push(px[1]);
                    rgb.push(px[2]);
                }
                let computed = compute_fine_grained_saliency(&rgb, w, h);
                let mut guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
                guard.dest_saliency = Some(computed.clone());
                computed
            }
        };

        let preview_img = render_saliency_preview(&dest, &sal_map, threshold.clamp(0.01, 0.99));
        let preview_data_url = encode_preview_data_url(&preview_img, PREVIEW_MAX_DIM)?;
        Ok(PreviewResponse {
            preview_data_url,
            width: w,
            height: h,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn generate_sorted(
    app: AppHandle,
    state: State<'_, AppState>,
    req: GenerateSortRequest,
) -> Result<GenerationResponse, String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let start = Instant::now();
        if req.ratio_w == 0 || req.ratio_h == 0 {
            return Err("Aspect ratio width and height must be greater than 0.".to_string());
        }

        let tiles = {
            let guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            if guard.tiles.is_empty() {
                return Err("Please load tiles first.".to_string());
            }
            guard.tiles.clone()
        };

        emit_progress(&app, "sort", 0, 1, "Sorting tiles...".to_string());
        emit_log(
            &app,
            "info",
            format!(
                "Generating sorted collage ({:?}, ratio {}:{})...",
                req.sort_method, req.ratio_w, req.ratio_h
            ),
        );

        let out = generate_sorted_collage(
            &tiles,
            (req.ratio_w, req.ratio_h),
            req.sort_method,
            req.rev_sort,
            req.rev_row,
        )?;

        let preview_data_url = encode_preview_data_url(&out.image, PREVIEW_MAX_DIM)?;
        let out_w = out.image.width();
        let out_h = out.image.height();
        let grid_w = out.grid_w;
        let grid_h = out.grid_h;

        {
            let mut guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard.raw_mosaic = None;
            guard.current_result = Some(out.image);
            guard.current_tile_info = Some(out.tile_info);
        }

        let elapsed_ms = start.elapsed().as_millis();
        emit_progress(&app, "done", 1, 1, "Sorted collage complete".to_string());
        emit_log(
            &app,
            "success",
            format!(
                "Sorted collage ({grid_w}x{grid_h} grid, {out_w}x{out_h} px) ready in {:.2}s.",
                elapsed_ms as f64 / 1000.0
            ),
        );

        Ok(GenerationResponse {
            grid_width: grid_w,
            grid_height: grid_h,
            output_width: out_w,
            output_height: out_h,
            preview_data_url,
            has_tile_info: true,
            elapsed_ms,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn generate_mosaic(
    app: AppHandle,
    state: State<'_, AppState>,
    req: GenerateMosaicRequest,
) -> Result<GenerationResponse, String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let start = Instant::now();
        let (tiles, dest, dest_has_alpha) = {
            let guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            if guard.tiles.is_empty() {
                return Err("Please load tiles first.".to_string());
            }
            let dest = guard
                .dest_img
                .clone()
                .ok_or_else(|| "Please select a target destination image first.".to_string())?;
            (guard.tiles.clone(), dest, guard.dest_has_alpha)
        };

        emit_log(
            &app,
            "info",
            format!(
                "Generating {:?} photomosaic ({:?} / {:?})...",
                req.fairness, req.colorspace, req.metric
            ),
        );

        let app_log = app.clone();
        let app_prog = app.clone();
        let out = generate_photomosaic(
            &tiles,
            &dest,
            dest_has_alpha,
            &req,
            move |lvl, msg| emit_log(&app_log, lvl, msg),
            move |stage, cur, tot, msg| emit_progress(&app_prog, stage, cur, tot, msg),
        )?;

        let final_img = blend_images(&out.image, &dest, req.blend_mode, req.blend_level);
        let preview_data_url = encode_preview_data_url(&final_img, PREVIEW_MAX_DIM)?;
        let out_w = final_img.width();
        let out_h = final_img.height();
        let grid_w = out.grid_w;
        let grid_h = out.grid_h;

        {
            let mut guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard.raw_mosaic = Some(out.image);
            guard.current_result = Some(final_img);
            guard.current_tile_info = Some(out.tile_info);
        }

        let elapsed_ms = start.elapsed().as_millis();
        emit_progress(&app, "done", 1, 1, "Photomosaic complete".to_string());
        emit_log(
            &app,
            "success",
            format!(
                "Photomosaic ({grid_w}x{grid_h} grid, {out_w}x{out_h} px) ready in {:.2}s.",
                elapsed_ms as f64 / 1000.0
            ),
        );

        Ok(GenerationResponse {
            grid_width: grid_w,
            grid_height: grid_h,
            output_width: out_w,
            output_height: out_h,
            preview_data_url,
            has_tile_info: true,
            elapsed_ms,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn apply_blend(
    state: State<'_, AppState>,
    mode: BlendMode,
    level: f32,
) -> Result<PreviewResponse, String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (raw, dest) = {
            let guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            let raw = guard
                .raw_mosaic
                .clone()
                .ok_or_else(|| "Generate a photomosaic first before blending.".to_string())?;
            let dest = guard
                .dest_img
                .clone()
                .ok_or_else(|| "No destination image loaded.".to_string())?;
            (raw, dest)
        };

        let blended = blend_images(&raw, &dest, mode, level);
        let (w, h) = blended.dimensions();
        let preview_data_url = encode_preview_data_url(&blended, PREVIEW_MAX_DIM)?;

        {
            let mut guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard.current_result = Some(blended);
        }

        Ok(PreviewResponse {
            preview_data_url,
            width: w,
            height: h,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn save_image(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let img = {
            let guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard
                .current_result
                .clone()
                .ok_or_else(|| "No image available to save yet.".to_string())?
        };

        let p = Path::new(&path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();

        if ext == "jpg" || ext == "jpeg" {
            // Composite alpha over white background for JPEG output
            let (w, h) = img.dimensions();
            let mut rgb_raw = Vec::with_capacity((w * h * 3) as usize);
            for px in img.pixels() {
                let a = px[3] as f32 / 255.0;
                let inv = 1.0 - a;
                rgb_raw.push((px[0] as f32 * a + 255.0 * inv).round() as u8);
                rgb_raw.push((px[1] as f32 * a + 255.0 * inv).round() as u8);
                rgb_raw.push((px[2] as f32 * a + 255.0 * inv).round() as u8);
            }
            let rgb_img = image::RgbImage::from_raw(w, h, rgb_raw).unwrap();
            DynamicImage::ImageRgb8(rgb_img)
                .save(p)
                .map_err(|e| format!("Failed to save JPEG: {e}"))?;
        } else {
            img.save(p)
                .map_err(|e| format!("Failed to save image: {e}"))?;
        }

        emit_log(&app, "success", format!("Saved image to {path}"));
        Ok(())
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn save_tile_info(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let info = {
            let guard = shared.lock().map_err(|_| "State lock poisoned".to_string())?;
            guard
                .current_tile_info
                .clone()
                .ok_or_else(|| "Generate a collage or photomosaic first.".to_string())?
        };

        std::fs::write(&path, info).map_err(|e| format!("Failed to save tile info: {e}"))?;
        emit_log(&app, "success", format!("Saved tile info to {path}"));
        Ok(())
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplePaths {
    pub tiles_dir: String,
    pub dest_img: String,
}

#[tauri::command]
pub fn get_sample_paths() -> Option<SamplePaths> {
    let cwd = std::env::current_dir().ok()?;
    for base in [cwd.clone(), cwd.parent().map(|p| p.to_path_buf()).unwrap_or(cwd)] {
        let ex_dir = base.join("examples");
        let dest = ex_dir.join("dest.jpg");
        if ex_dir.is_dir() && dest.is_file() {
            return Some(SamplePaths {
                tiles_dir: ex_dir.to_string_lossy().into_owned(),
                dest_img: dest.to_string_lossy().into_owned(),
            });
        }
    }
    None
}

