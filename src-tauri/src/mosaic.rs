use crate::colorspace::{
    compute_sort_key, convert_rgb_buffer, hls_to_rgb, rgb_to_hls, FeatureIndex,
};
use crate::lapjv::solve_lapjv;
use crate::saliency::compute_fine_grained_saliency;
use crate::types::{
    BlendMode, ColorSpace, DistanceMetric, FairnessMode, GenerateMosaicRequest, SortMethod,
    TileImage,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::{
    imageops::FilterType, DynamicImage, ImageFormat, RgbImage, Rgba, RgbaImage,
};
use rand::seq::SliceRandom;
use rayon::prelude::*;
use std::io::Cursor;

pub struct MosaicOutput {
    pub image: RgbaImage,
    pub tile_info: String,
    pub grid_w: u32,
    pub grid_h: u32,
}

/// Find the (grid_w, grid_h) whose overall pixel aspect ratio best matches (rw / rh).
pub fn calc_grid_size(rw: u32, rh: u32, num_imgs: usize, tw: u32, th: u32) -> (u32, u32) {
    if num_imgs <= 1 {
        return (1, 1);
    }
    let dest_ratio = rw as f64 / rh.max(1) as f64;
    let mut best_grid = (1u32, num_imgs as u32);
    let mut best_err = f64::INFINITY;

    for width in 1..=num_imgs {
        let height = (num_imgs + width - 1) / width;
        let ratio = (width as f64 * tw as f64) / (height as f64 * th as f64);
        let err = (ratio - dest_ratio).powi(2);
        if err < best_err {
            best_err = err;
            best_grid = (width as u32, height as u32);
        }
    }
    best_grid
}

/// Blit RGB tile bytes into RGBA output canvas at tile coordinate (col, row).
#[inline]
fn blit_tile(canvas: &mut RgbaImage, tile: &TileImage, col: u32, row: u32) {
    let tw = tile.width;
    let th = tile.height;
    let canvas_w = canvas.width() as usize;
    let dst_buf = canvas.as_mut();
    let ox = (col * tw) as usize;
    let oy = (row * th) as usize;

    for y in 0..th as usize {
        let src_row = y * tw as usize * 3;
        let dst_row = ((oy + y) * canvas_w + ox) * 4;
        for x in 0..tw as usize {
            let si = src_row + x * 3;
            let di = dst_row + x * 4;
            dst_buf[di] = tile.rgb[si];
            dst_buf[di + 1] = tile.rgb[si + 1];
            dst_buf[di + 2] = tile.rgb[si + 2];
            dst_buf[di + 3] = 255;
        }
    }
}

/// Assemble a full grid collage from an ordered slice of tiles (with optional transparent padding).
pub fn make_collage_grid(
    grid: (u32, u32),
    tiles: &[&TileImage],
    rev_row: bool,
) -> MosaicOutput {
    let (gw, gh) = grid;
    let total = (gw as usize) * (gh as usize);
    let tw = tiles[0].width;
    let th = tiles[0].height;

    let mut canvas = RgbaImage::from_pixel(gw * tw, gh * th, Rgba([255, 255, 255, 0]));
    let mut info_lines = Vec::with_capacity(total + 1);
    info_lines.push(format!("Grid dimension: ({gw}, {gh})"));

    for row in 0..gh {
        for col in 0..gw {
            let logical_col = if rev_row && (row % 2 == 1) {
                gw - 1 - col
            } else {
                col
            };
            let idx = (row * gw + logical_col) as usize;
            if idx < tiles.len() {
                let tile = tiles[idx];
                blit_tile(&mut canvas, tile, col, row);
                info_lines.push(tile.path.clone());
            } else {
                info_lines.push("background".to_string());
            }
        }
    }

    MosaicOutput {
        image: canvas,
        tile_info: info_lines.join("\n"),
        grid_w: gw,
        grid_h: gh,
    }
}

/// Assemble a masked collage where only specified `(row, col)` cells receive tiles.
fn make_collage_masked(
    grid: (u32, u32),
    assigned_tiles: &[&TileImage],
    coords: &[(u32, u32)],
) -> MosaicOutput {
    let (gw, gh) = grid;
    let tw = assigned_tiles[0].width;
    let th = assigned_tiles[0].height;

    let mut canvas = RgbaImage::from_pixel(gw * tw, gh * th, Rgba([255, 255, 255, 0]));
    let mut grid_info = vec!["background".to_string(); (gw * gh) as usize];

    for (tile, &(row, col)) in assigned_tiles.iter().zip(coords.iter()) {
        blit_tile(&mut canvas, tile, col, row);
        grid_info[(row * gw + col) as usize] = tile.path.clone();
    }

    let mut out_info = String::with_capacity(grid_info.len() * 32);
    out_info.push_str(&format!("Grid dimension: ({gw}, {gh})\n"));
    out_info.push_str(&grid_info.join("\n"));

    MosaicOutput {
        image: canvas,
        tile_info: out_info,
        grid_w: gw,
        grid_h: gh,
    }
}

/// Sort tiles and build a collage with the requested aspect ratio.
pub fn generate_sorted_collage(
    tiles: &[TileImage],
    ratio: (u32, u32),
    method: SortMethod,
    rev_sort: bool,
    rev_row: bool,
) -> Result<MosaicOutput, String> {
    if tiles.is_empty() {
        return Err("No tiles loaded.".to_string());
    }
    let grid = calc_grid_size(ratio.0, ratio.1, tiles.len(), tiles[0].width, tiles[0].height);

    let mut indices: Vec<usize> = (0..tiles.len()).collect();
    if method != SortMethod::None {
        let keys: Vec<f64> = tiles
            .par_iter()
            .map(|t| compute_sort_key(t, method))
            .collect();
        indices.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]));
        if rev_sort {
            indices.reverse();
        }
    }

    let ordered: Vec<&TileImage> = indices.into_iter().map(|i| &tiles[i]).collect();
    Ok(make_collage_grid(grid, &ordered, rev_row))
}

/// Duplicate or slice tile references to match an exact target cell count `total`.
fn dup_tile_indices(num_tiles: usize, total: usize) -> (Vec<usize>, String) {
    if total <= num_tiles {
        let msg = format!(
            "{total} tiles will be used 1 time. {}/{num_tiles} tiles will not be used.",
            num_tiles - total
        );
        return ((0..total).collect(), msg);
    }

    let full_count = total / num_tiles;
    let remaining = total % num_tiles;
    let mut indices = Vec::with_capacity(total);
    for _ in 0..full_count {
        indices.extend(0..num_tiles);
    }
    indices.extend(0..remaining);

    let msg = if remaining > 0 {
        format!(
            "{} tiles used {full_count}x, {remaining} tiles used {}x (from {num_tiles} unique tiles).",
            num_tiles - remaining,
            full_count + 1
        )
    } else {
        format!("All {num_tiles} tiles will be used exactly {full_count}x.")
    };
    (indices, msg)
}

fn compute_block_size(dest_w: u32, dest_h: u32, grid: (u32, u32), tw: u32, th: u32) -> (u32, u32) {
    let mut bw = ((dest_w as f64) / (grid.0 as f64)).round().max(1.0) as u32;
    let mut bh = ((dest_h as f64) / (grid.1 as f64)).round().max(1.0) as u32;

    if bw > tw || bh > th {
        let m = (tw as f64 / bw as f64).max(th as f64 / bh as f64);
        bw = ((bw as f64) * m).floor().max(1.0) as u32;
        bh = ((bh as f64) * m).floor().max(1.0) as u32;
    }
    (bw.max(1), bh.max(1))
}

/// Resize a tile's RGB buffer to (bw, bh) and convert to flat colorspace features.
fn build_tile_features(
    tiles: &[&TileImage],
    bw: u32,
    bh: u32,
    colorspace: ColorSpace,
    metric: DistanceMetric,
) -> FeatureIndex {
    let dim = (bw * bh * 3) as usize;
    let per_tile: Vec<Vec<f32>> = tiles
        .par_iter()
        .map(|tile| {
            let resized_rgb = if tile.width == bw && tile.height == bh {
                tile.rgb.clone()
            } else {
                let rgb_img =
                    RgbImage::from_raw(tile.width, tile.height, tile.rgb.clone()).unwrap();
                DynamicImage::ImageRgb8(rgb_img)
                    .resize_exact(bw, bh, FilterType::Triangle)
                    .to_rgb8()
                    .into_raw()
            };
            convert_rgb_buffer(&resized_rgb, colorspace)
        })
        .collect();

    let mut flat = Vec::with_capacity(tiles.len() * dim);
    for v in per_tile {
        flat.extend_from_slice(&v);
    }
    FeatureIndex::new(metric, flat, dim)
}

/// Extract flat block feature vectors from an RGB image of size `(grid_w * bw, grid_h * bh)`.
fn extract_dest_blocks(
    rgb: &[u8],
    grid: (u32, u32),
    bw: u32,
    bh: u32,
    colorspace: ColorSpace,
    mask_coords: Option<&[(u32, u32)]>,
) -> Vec<Vec<f32>> {
    let full_w = (grid.0 * bw) as usize;
    let features = convert_rgb_buffer(rgb, colorspace);
    let dim = (bw * bh * 3) as usize;

    let coords: Vec<(u32, u32)> = match mask_coords {
        Some(c) => c.to_vec(),
        None => {
            let mut v = Vec::with_capacity((grid.0 * grid.1) as usize);
            for r in 0..grid.1 {
                for c in 0..grid.0 {
                    v.push((r, c));
                }
            }
            v
        }
    };

    coords
        .par_iter()
        .map(|&(row, col)| {
            let mut block = Vec::with_capacity(dim);
            let oy = (row * bh) as usize;
            let ox = (col * bw) as usize;
            for y in 0..bh as usize {
                let start = ((oy + y) * full_w + ox) * 3;
                block.extend_from_slice(&features[start..start + bw as usize * 3]);
            }
            block
        })
        .collect()
}

/// Find `(row, col)` block coordinates where the maximum value of `thresh_map` in the block >= `lower_thresh`.
fn compute_block_mask(
    thresh_map: &[f32],
    map_w: u32,
    map_h: u32,
    bw: u32,
    bh: u32,
    lower_thresh: f32,
) -> (Vec<(u32, u32)>, (u32, u32), u32, u32) {
    let gw = (map_w / bw).max(1);
    let gh = (map_h / bh).max(1);
    let eff_w = gw * bw;
    let eff_h = gh * bh;
    let mw = map_w as usize;

    let mut coords = Vec::new();
    for r in 0..gh {
        for c in 0..gw {
            let mut max_v = 0.0f32;
            let oy = (r * bh) as usize;
            let ox = (c * bw) as usize;
            'block: for y in oy..oy + bh as usize {
                let row_off = y * mw;
                for x in ox..ox + bw as usize {
                    let v = thresh_map[row_off + x];
                    if v > max_v {
                        max_v = v;
                        if max_v >= lower_thresh {
                            break 'block;
                        }
                    }
                }
            }
            if max_v >= lower_thresh {
                coords.push((r, c));
            }
        }
    }
    (coords, (gw, gh), eff_w, eff_h)
}

fn rgba_to_rgb(img: &RgbaImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for px in img.pixels() {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
    }
    RgbImage::from_raw(w, h, rgb).unwrap()
}

fn alpha_mask(img: &RgbaImage) -> Vec<f32> {
    img.pixels()
        .map(|px| if px[3] > 0 { 1.0 } else { 0.0 })
        .collect()
}

/// Generate a photomosaic (Fair or Unfair) with progress and log callbacks.
pub fn generate_photomosaic<FL, FP>(
    tiles: &[TileImage],
    dest: &RgbaImage,
    dest_has_alpha: bool,
    req: &GenerateMosaicRequest,
    log: FL,
    progress: FP,
) -> Result<MosaicOutput, String>
where
    FL: Fn(&str, String) + Sync,
    FP: Fn(&str, usize, usize, String) + Sync,
{
    if tiles.is_empty() {
        return Err("No tiles loaded.".to_string());
    }
    if req.transparent && !dest_has_alpha {
        return Err(
            "Transparency masking requires a destination image with an alpha channel (e.g. transparent PNG)."
                .to_string(),
        );
    }

    let (dest_w, dest_h) = dest.dimensions();
    let tw = tiles[0].width;
    let th = tiles[0].height;

    match req.fairness {
        FairnessMode::Fair => {
            if !(req.dup > 0.0 && req.dup.is_finite()) {
                return Err("Duplicates factor must be a positive number.".to_string());
            }

            if req.salient || req.transparent {
                generate_fair_masked(tiles, dest, req, &log, &progress)
            } else {
                let target_total = ((tiles.len() as f64) * req.dup).round().max(1.0) as usize;
                let grid = calc_grid_size(dest_w, dest_h, target_total, tw, th);
                let total = (grid.0 * grid.1) as usize;
                log(
                    "info",
                    format!(
                        "Grid: {}x{} ({total} cells) | Output: {}x{}",
                        grid.0,
                        grid.1,
                        grid.0 * tw,
                        grid.1 * th
                    ),
                );
                if total > 4000 {
                    log(
                        "warn",
                        format!("Large fair assignment ({total}x{total}). This may take a while."),
                    );
                }

                let (tile_indices, dup_msg) = dup_tile_indices(tiles.len(), total);
                log("info", dup_msg);

                let (bw, bh) = compute_block_size(dest_w, dest_h, grid, tw, th);
                let target_w = grid.0 * bw;
                let target_h = grid.1 * bh;
                log("info", format!("Block size: {bw}x{bh} | Target resized to {target_w}x{target_h}"));

                progress("features", 0, 2, "Extracting tile features...".to_string());
                // Deduplicate feature extraction on unique tiles first, then index!
                let unique_refs: Vec<&TileImage> = tiles.iter().collect();
                let unique_idx = build_tile_features(&unique_refs, bw, bh, req.colorspace, req.metric);

                let resized_dest = DynamicImage::ImageRgb8(rgba_to_rgb(dest))
                    .resize_exact(target_w, target_h, FilterType::Triangle)
                    .to_rgb8();
                let dest_blocks =
                    extract_dest_blocks(resized_dest.as_raw(), grid, bw, bh, req.colorspace, None);
                progress("features", 1, 2, "Computing pairwise distance matrix...".to_string());

                // Build square cost matrix [block][duplicated_tile_slot]
                let n = total;
                let num_unique = tiles.len();
                let unique_dists: Vec<Vec<f32>> = dest_blocks
                    .par_iter()
                    .map(|block| {
                        let mut d = vec![0.0f32; num_unique];
                        unique_idx.distances_to(block, &mut d);
                        d
                    })
                    .collect();

                let mut cost_matrix = vec![0.0f32; n * n];
                cost_matrix
                    .par_chunks_mut(n)
                    .enumerate()
                    .for_each(|(b, row)| {
                        let ud = &unique_dists[b];
                        for (slot, &u_idx) in tile_indices.iter().enumerate() {
                            row[slot] = ud[u_idx];
                        }
                    });
                progress("features", 2, 2, "Distance matrix ready".to_string());

                log("info", format!("Solving {n}x{n} Jonker-Volgenant optimal assignment..."));
                let (rowsol, total_cost) = solve_lapjv(&cost_matrix, n, |cur, tot| {
                    progress(
                        "lapjv",
                        cur,
                        tot,
                        format!("Solving optimal assignment ({cur}/{tot})..."),
                    );
                });
                log("info", format!("Optimal assignment cost: {:.2}", total_cost));

                let assigned: Vec<&TileImage> = rowsol
                    .into_iter()
                    .map(|slot| &tiles[tile_indices[slot]])
                    .collect();
                Ok(make_collage_grid(grid, &assigned, false))
            }
        }
        FairnessMode::Unfair => generate_unfair(tiles, dest, req, &log, &progress),
    }
}

fn generate_fair_masked<FL, FP>(
    tiles: &[TileImage],
    dest: &RgbaImage,
    req: &GenerateMosaicRequest,
    log: &FL,
    progress: &FP,
) -> Result<MosaicOutput, String>
where
    FL: Fn(&str, String) + Sync,
    FP: Fn(&str, usize, usize, String) + Sync,
{
    let (dest_w, dest_h) = dest.dimensions();
    let tw = tiles[0].width;
    let th = tiles[0].height;
    let total = ((tiles.len() as f64) * req.dup).round().max(1.0) as usize;
    let init_grid = calc_grid_size(dest_w, dest_h, total, tw, th);

    let dest_rgb = rgba_to_rgb(dest);
    let (orig_map, lower_thresh) = if req.transparent {
        (alpha_mask(dest), 0.5f32)
    } else {
        log("info", "Computing fine-grained saliency map...".to_string());
        (
            compute_fine_grained_saliency(dest_rgb.as_raw(), dest_w, dest_h),
            req.lower_thresh,
        )
    };

    // DDA-like loop to shrink block size until masked cells >= total (matching calc_salient_col_even)
    let mut bw_f = dest_w as f64 / init_grid.0 as f64;
    let mut bh_f = dest_h as f64 / init_grid.1 as f64;
    let (bw_delta, bh_delta) = if bw_f > bh_f {
        (1.0, bh_f / bw_f)
    } else {
        (bw_f / bh_f, 1.0)
    };

    let (coords, grid, bw, bh, eff_w, eff_h) = loop {
        let bw = (bw_f as u32).max(1);
        let bh = (bh_f as u32).max(1);
        let (coords, grid, ew, eh) =
            compute_block_mask(&orig_map, dest_w, dest_h, bw, bh, lower_thresh);
        if coords.len() >= total || bw <= 1 || bh <= 1 {
            if coords.len() < total {
                log(
                    "warn",
                    "Masked area is small relative to tile count; using finest block resolution."
                        .to_string(),
                );
            }
            break (coords, grid, bw, bh, ew, eh);
        }
        bw_f -= bw_delta;
        bh_f -= bh_delta;
    };

    if coords.is_empty() {
        return Err("No pixels passed the saliency/transparency mask threshold. Try lowering the threshold.".to_string());
    }

    let n = coords.len();
    log(
        "info",
        format!(
            "Masked blocks: {n}/{} | Grid: {}x{} | Block: {bw}x{bh}",
            grid.0 * grid.1,
            grid.0,
            grid.1
        ),
    );
    let (tile_indices, dup_msg) = dup_tile_indices(tiles.len(), n);
    log("info", dup_msg);

    let unique_refs: Vec<&TileImage> = tiles.iter().collect();
    let unique_idx = build_tile_features(&unique_refs, bw, bh, req.colorspace, req.metric);

    let cropped_dest = DynamicImage::ImageRgb8(dest_rgb)
        .crop_imm(0, 0, eff_w, eff_h)
        .to_rgb8();
    let dest_blocks = extract_dest_blocks(
        cropped_dest.as_raw(),
        grid,
        bw,
        bh,
        req.colorspace,
        Some(&coords),
    );

    let num_unique = tiles.len();
    let unique_dists: Vec<Vec<f32>> = dest_blocks
        .par_iter()
        .map(|block| {
            let mut d = vec![0.0f32; num_unique];
            unique_idx.distances_to(block, &mut d);
            d
        })
        .collect();

    let mut cost_matrix = vec![0.0f32; n * n];
    cost_matrix
        .par_chunks_mut(n)
        .enumerate()
        .for_each(|(b, row)| {
            let ud = &unique_dists[b];
            for (slot, &u_idx) in tile_indices.iter().enumerate() {
                row[slot] = ud[u_idx];
            }
        });

    log("info", format!("Solving {n}x{n} optimal masked assignment..."));
    let (rowsol, total_cost) = solve_lapjv(&cost_matrix, n, |cur, tot| {
        progress(
            "lapjv",
            cur,
            tot,
            format!("Solving optimal assignment ({cur}/{tot})..."),
        );
    });
    log("info", format!("Optimal assignment cost: {:.2}", total_cost));

    let assigned: Vec<&TileImage> = rowsol
        .into_iter()
        .map(|slot| &tiles[tile_indices[slot]])
        .collect();
    Ok(make_collage_masked(grid, &assigned, &coords))
}

fn generate_unfair<FL, FP>(
    tiles: &[TileImage],
    dest: &RgbaImage,
    req: &GenerateMosaicRequest,
    log: &FL,
    progress: &FP,
) -> Result<MosaicOutput, String>
where
    FL: Fn(&str, String) + Sync,
    FP: Fn(&str, usize, usize, String) + Sync,
{
    if req.max_width == 0 {
        return Err("Max width must be at least 1.".to_string());
    }
    if req.freq_mul < 0.0 || !req.freq_mul.is_finite() {
        return Err("Frequency multiplier must be >= 0.".to_string());
    }

    let (dest_w, dest_h) = dest.dimensions();
    let tw = tiles[0].width;
    let th = tiles[0].height;

    let grid_w = req.max_width;
    let grid_h = ((dest_h as f64) * (grid_w as f64 * tw as f64 / dest_w as f64) / th as f64)
        .round()
        .max(1.0) as u32;
    let grid = (grid_w, grid_h);

    let (bw, bh) = compute_block_size(dest_w, dest_h, grid, tw, th);
    let target_w = grid.0 * bw;
    let target_h = grid.1 * bh;
    log(
        "info",
        format!(
            "Grid: {}x{} | Output: {}x{} | Block: {bw}x{bh}",
            grid.0,
            grid.1,
            grid.0 * tw,
            grid.1 * th
        ),
    );

    let use_transparent = req.transparent;
    let use_dither = req.dither && !use_transparent;
    let use_saliency = req.salient && !use_transparent && !use_dither;
    let randomize = !req.deterministic && !use_dither;

    if req.dither && use_transparent {
        log("warn", "Dithering disabled because transparency masking is active.".to_string());
    }
    if req.salient && (use_transparent || use_dither) {
        log("warn", "Saliency disabled because transparency or dithering is active.".to_string());
    }

    progress("features", 0, 2, "Extracting tile features...".to_string());
    let tile_refs: Vec<&TileImage> = tiles.iter().collect();
    let feat_index = build_tile_features(&tile_refs, bw, bh, req.colorspace, req.metric);

    let resized_rgba = DynamicImage::ImageRgba8(dest.clone())
        .resize_exact(target_w, target_h, FilterType::Triangle)
        .to_rgba8();
    let resized_rgb = rgba_to_rgb(&resized_rgba);

    let mask_coords: Option<Vec<(u32, u32)>> = if use_saliency || use_transparent {
        let (tmap, thresh) = if use_transparent {
            (alpha_mask(&resized_rgba), 0.5f32)
        } else {
            log("info", "Computing fine-grained saliency map...".to_string());
            (
                compute_fine_grained_saliency(resized_rgb.as_raw(), target_w, target_h),
                req.lower_thresh,
            )
        };
        let (coords, _, _, _) = compute_block_mask(&tmap, target_w, target_h, bw, bh, thresh);
        if coords.is_empty() {
            return Err("No pixels passed the saliency/transparency mask threshold. Try lowering the threshold.".to_string());
        }
        log(
            "info",
            format!("Masked blocks: {}/{}", coords.len(), grid.0 * grid.1),
        );
        Some(coords)
    } else {
        None
    };

    let mut dest_blocks = extract_dest_blocks(
        resized_rgb.as_raw(),
        grid,
        bw,
        bh,
        req.colorspace,
        mask_coords.as_deref(),
    );
    let total = dest_blocks.len();
    let num_tiles = tiles.len();
    progress("assign", 0, total, "Matching tiles...".to_string());

    let assignment: Vec<usize> = if use_dither {
        // Floyd-Steinberg error diffusion over grid (gw x gh)
        let gw = grid.0 as usize;
        let gh = grid.1 as usize;
        let dim = feat_index.dim;
        let mut out = vec![0usize; total];
        let mut dist = vec![0.0f32; num_tiles];
        let mut freq = vec![0.0f32; num_tiles];
        let mut rank_order: Vec<usize> = (0..num_tiles).collect();
        let mut rank_scores = vec![0.0f32; num_tiles];
        let mut err_buf = vec![0.0f32; dim];

        let pick_tile = |block: &[f32],
                         dist: &mut [f32],
                         freq: &mut [f32],
                         rank_order: &mut [usize],
                         rank_scores: &mut [f32]|
         -> usize {
            feat_index.distances_to(block, dist);
            if req.freq_mul > 0.0 {
                for (i, slot) in rank_order.iter_mut().enumerate() {
                    *slot = i;
                }
                rank_order.sort_by(|&a, &b| dist[a].total_cmp(&dist[b]));
                for (rank, &tile_i) in rank_order.iter().enumerate() {
                    rank_scores[tile_i] = rank as f32 + freq[tile_i];
                }
                let best = rank_scores
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                freq[best] += req.freq_mul;
                best
            } else {
                dist.iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        };

        let step = (total / 40).max(1);
        for r in 0..gh {
            for c in 0..gw {
                let idx = r * gw + c;
                let best = pick_tile(
                    &dest_blocks[idx],
                    &mut dist,
                    &mut freq,
                    &mut rank_order,
                    &mut rank_scores,
                );
                out[idx] = best;

                // Diffuse quantization error to neighbors (right, bottom-left, bottom, bottom-right)
                let tile_v = feat_index.tile_vec(best);
                let cur_v = &dest_blocks[idx];
                for k in 0..dim {
                    err_buf[k] = cur_v[k] - tile_v[k];
                }

                if c + 1 < gw {
                    let nb = &mut dest_blocks[idx + 1];
                    for k in 0..dim {
                        nb[k] += err_buf[k] * 0.4375;
                    }
                }
                if r + 1 < gh {
                    if c > 0 {
                        let nb = &mut dest_blocks[idx + gw - 1];
                        for k in 0..dim {
                            nb[k] += err_buf[k] * 0.1875;
                        }
                    }
                    {
                        let nb = &mut dest_blocks[idx + gw];
                        for k in 0..dim {
                            nb[k] += err_buf[k] * 0.3125;
                        }
                    }
                    if c + 1 < gw {
                        let nb = &mut dest_blocks[idx + gw + 1];
                        for k in 0..dim {
                            nb[k] += err_buf[k] * 0.0625;
                        }
                    }
                }

                if idx % step == 0 || idx + 1 == total {
                    progress("assign", idx + 1, total, format!("Dithering ({}/{total})...", idx + 1));
                }
            }
        }
        out
    } else if req.freq_mul > 0.0 {
        // Compute all distance vectors in parallel, then apply sequential frequency-penalized selection
        let mut order: Vec<usize> = (0..total).collect();
        if randomize {
            order.shuffle(&mut rand::thread_rng());
        }

        // Compute rank lists in parallel in chunks to keep memory bounded
        let chunk_size = 512usize;
        let mut out = vec![0usize; total];
        let mut freq = vec![0.0f32; num_tiles];
        let mut processed = 0usize;

        for chunk in order.chunks(chunk_size) {
            let chunk_ranks: Vec<Vec<usize>> = chunk
                .par_iter()
                .map(|&block_idx| {
                    let mut dist = vec![0.0f32; num_tiles];
                    feat_index.distances_to(&dest_blocks[block_idx], &mut dist);
                    let mut rank_order: Vec<usize> = (0..num_tiles).collect();
                    rank_order.sort_by(|&a, &b| dist[a].total_cmp(&dist[b]));
                    rank_order
                })
                .collect();

            let mut scores = vec![0.0f32; num_tiles];
            for (&block_idx, rank_order) in chunk.iter().zip(chunk_ranks.iter()) {
                for (rank, &tile_i) in rank_order.iter().enumerate() {
                    scores[tile_i] = rank as f32 + freq[tile_i];
                }
                let best = scores
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                out[block_idx] = best;
                freq[best] += req.freq_mul;
            }

            processed += chunk.len();
            progress(
                "assign",
                processed,
                total,
                format!("Matching with fairness penalty ({processed}/{total})..."),
            );
        }
        out
    } else {
        // Pure best-fit: embarrassingly parallel via rayon!
        let out: Vec<usize> = dest_blocks
            .par_iter()
            .map(|block| {
                let mut dist = vec![0.0f32; num_tiles];
                feat_index.distances_to(block, &mut dist);
                dist.iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            })
            .collect();
        progress("assign", total, total, "Best-fit matching complete".to_string());
        out
    };

    let assigned: Vec<&TileImage> = assignment.into_iter().map(|i| &tiles[i]).collect();
    if let Some(coords) = mask_coords {
        Ok(make_collage_masked(grid, &assigned, &coords))
    } else {
        Ok(make_collage_grid(grid, &assigned, false))
    }
}

/// Apply alpha or HSL lightness blending of `mosaic` with `dest`.
/// `level` is in `[0.0, 1.0]` where `0.0` = 100% mosaic and `1.0` = 100% target blend.
pub fn blend_images(
    mosaic: &RgbaImage,
    dest: &RgbaImage,
    mode: BlendMode,
    level: f32,
) -> RgbaImage {
    let level = level.clamp(0.0, 1.0);
    if level <= 1e-4 {
        return mosaic.clone();
    }

    let (w, h) = mosaic.dimensions();
    let alpha = 1.0 - level;
    let resized_dest = DynamicImage::ImageRgba8(dest.clone())
        .resize_exact(w, h, FilterType::Triangle)
        .to_rgb8();

    let src_raw = mosaic.as_raw();
    let dst_raw = resized_dest.as_raw();
    let mut out_raw = vec![0u8; src_raw.len()];

    match mode {
        BlendMode::Alpha => {
            out_raw
                .par_chunks_mut(4)
                .enumerate()
                .for_each(|(i, px)| {
                    let si = i * 4;
                    let di = i * 3;
                    px[0] = (src_raw[si] as f32 * alpha + dst_raw[di] as f32 * level)
                        .clamp(0.0, 255.0) as u8;
                    px[1] = (src_raw[si + 1] as f32 * alpha + dst_raw[di + 1] as f32 * level)
                        .clamp(0.0, 255.0) as u8;
                    px[2] = (src_raw[si + 2] as f32 * alpha + dst_raw[di + 2] as f32 * level)
                        .clamp(0.0, 255.0) as u8;
                    px[3] = src_raw[si + 3];
                });
        }
        BlendMode::Brightness => {
            const INV_255: f32 = 1.0 / 255.0;
            out_raw
                .par_chunks_mut(4)
                .enumerate()
                .for_each(|(i, px)| {
                    let si = i * 4;
                    let di = i * 3;
                    let [h_m, l_m, s_m] = rgb_to_hls(
                        src_raw[si] as f32 * INV_255,
                        src_raw[si + 1] as f32 * INV_255,
                        src_raw[si + 2] as f32 * INV_255,
                    );
                    let [_, l_d, _] = rgb_to_hls(
                        dst_raw[di] as f32 * INV_255,
                        dst_raw[di + 1] as f32 * INV_255,
                        dst_raw[di + 2] as f32 * INV_255,
                    );
                    let l_new = (l_m * alpha + l_d * level).clamp(0.0, 1.0);
                    let [r, g, b] = hls_to_rgb(h_m, l_new, s_m);
                    px[0] = (r * 255.0).round().clamp(0.0, 255.0) as u8;
                    px[1] = (g * 255.0).round().clamp(0.0, 255.0) as u8;
                    px[2] = (b * 255.0).round().clamp(0.0, 255.0) as u8;
                    px[3] = src_raw[si + 3];
                });
        }
    }

    RgbaImage::from_raw(w, h, out_raw).unwrap()
}

/// Render a saliency-masked preview of the destination image (non-salient regions faded/masked white).
pub fn render_saliency_preview(dest: &RgbaImage, saliency_map: &[f32], threshold: f32) -> RgbaImage {
    let (w, h) = dest.dimensions();
    let mut out = dest.clone();
    for (i, px) in out.pixels_mut().enumerate() {
        if saliency_map[i] < threshold {
            // Subtle white mask matching gui.py's visual threshold feedback
            px[0] = 245;
            px[1] = 245;
            px[2] = 248;
            px[3] = 255;
        }
    }
    RgbaImage::from_raw(w, h, out.into_raw()).unwrap()
}

/// Encode an `RgbaImage` as a base64 PNG data URL, downscaling if larger than `max_dim` for crisp UI performance.
pub fn encode_preview_data_url(img: &RgbaImage, max_dim: u32) -> Result<String, String> {
    let (w, h) = img.dimensions();
    let preview_dyn = if w > max_dim || h > max_dim {
        let scale = (max_dim as f64 / w as f64).min(max_dim as f64 / h as f64);
        let nw = ((w as f64) * scale).round().max(1.0) as u32;
        let nh = ((h as f64) * scale).round().max(1.0) as u32;
        DynamicImage::ImageRgba8(img.clone()).resize_exact(nw, nh, FilterType::Triangle)
    } else {
        DynamicImage::ImageRgba8(img.clone())
    };

    let mut buf = Cursor::new(Vec::new());
    preview_dyn
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("Failed to encode preview PNG: {e}"))?;

    Ok(format!("data:image/png;base64,{}", BASE64.encode(buf.into_inner())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BlendMode, ColorSpace, DistanceMetric, FairnessMode, SortMethod};

    fn make_solid_tile(r: u8, g: u8, b: u8, name: &str) -> TileImage {
        let mut rgb = Vec::with_capacity(8 * 8 * 3);
        for _ in 0..64 {
            rgb.push(r);
            rgb.push(g);
            rgb.push(b);
        }
        TileImage {
            width: 8,
            height: 8,
            rgb,
            path: name.to_string(),
        }
    }

    fn make_test_dest() -> RgbaImage {
        let mut img = RgbaImage::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                let r = (x * 8).min(255) as u8;
                let g = (y * 8).min(255) as u8;
                let b = 120u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
        img
    }

    #[test]
    fn test_sort_collage_and_fair_unfair_mosaics() {
        let tiles = vec![
            make_solid_tile(240, 20, 20, "red.png"),
            make_solid_tile(20, 240, 20, "green.png"),
            make_solid_tile(20, 20, 240, "blue.png"),
            make_solid_tile(240, 240, 20, "yellow.png"),
            make_solid_tile(20, 240, 240, "cyan.png"),
            make_solid_tile(240, 20, 240, "magenta.png"),
            make_solid_tile(240, 240, 240, "white.png"),
            make_solid_tile(15, 15, 15, "black.png"),
        ];
        let dest = make_test_dest();

        // 1. Sort mode
        let sorted = generate_sorted_collage(&tiles, (4, 2), SortMethod::BgrSum, false, true)
            .expect("sort collage");
        assert_eq!(sorted.image.width(), sorted.grid_w * 8);
        assert_eq!(sorted.image.height(), sorted.grid_h * 8);

        // 2. Fair mosaic mode (LAPJV)
        let fair_req = GenerateMosaicRequest {
            colorspace: ColorSpace::Lab,
            metric: DistanceMetric::Euclidean,
            fairness: FairnessMode::Fair,
            dup: 2.0,
            max_width: 8,
            freq_mul: 0.0,
            dither: false,
            deterministic: false,
            salient: false,
            lower_thresh: 0.5,
            transparent: false,
            blend_mode: BlendMode::Brightness,
            blend_level: 0.25,
        };
        let fair = generate_photomosaic(&tiles, &dest, false, &fair_req, |_, _| {}, |_, _, _, _| {})
            .expect("fair mosaic");
        assert_eq!(fair.grid_w, 4);
        assert_eq!(fair.grid_h, 4);

        // 3. Unfair mode with dithering + freq_mul
        let unfair_req = GenerateMosaicRequest {
            fairness: FairnessMode::Unfair,
            max_width: 6,
            freq_mul: 1.5,
            dither: true,
            deterministic: true,
            ..fair_req
        };
        let unfair =
            generate_photomosaic(&tiles, &dest, false, &unfair_req, |_, _| {}, |_, _, _, _| {})
                .expect("unfair dither mosaic");
        assert_eq!(unfair.grid_w, 6);

        // 4. Blending modes
        let alpha_out = blend_images(&fair.image, &dest, BlendMode::Alpha, 0.3);
        assert_eq!(alpha_out.dimensions(), fair.image.dimensions());
        let bright_out = blend_images(&fair.image, &dest, BlendMode::Brightness, 0.3);
        assert_eq!(bright_out.dimensions(), fair.image.dimensions());
    }
}
