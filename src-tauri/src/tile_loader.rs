use crate::types::{ResizeOption, TileImage};
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgb, RgbImage};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

fn collect_files(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if !dir.is_dir() {
        return Err(format!("Directory does not exist: {}", dir.display()));
    }
    let walker = if recursive {
        WalkDir::new(dir)
    } else {
        WalkDir::new(dir).max_depth(1)
    };

    let files: Vec<PathBuf> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    if files.is_empty() {
        return Err(format!("No files found in {}", dir.display()));
    }
    Ok(files)
}

fn fast_image_size(path: &Path) -> Option<(usize, usize)> {
    if let Ok(dim) = imagesize::size(path) {
        if dim.width > 0 && dim.height > 0 {
            return Some((dim.width, dim.height));
        }
    }
    if let Ok((w, h)) = image::image_dimensions(path) {
        if w > 0 && h > 0 {
            return Some((w as usize, h as usize));
        }
    }
    None
}

fn infer_tile_height(files: &[PathBuf], tile_width: u32) -> Result<u32, String> {
    let dims: Vec<(usize, usize)> = files
        .par_iter()
        .filter_map(|p| fast_image_size(p))
        .collect();

    if dims.is_empty() {
        return Err(
            "Failed to infer tile height: no readable image files were found in the folder."
                .to_string(),
        );
    }

    let mut counts: HashMap<(usize, usize), usize> = HashMap::new();
    for (w, h) in dims {
        let g = gcd(w, h);
        *counts.entry((w / g, h / g)).or_insert(0) += 1;
    }

    let ((rw, rh), _) = counts
        .into_iter()
        .max_by_key(|&(_, c)| c)
        .ok_or_else(|| "Failed to infer tile aspect ratio".to_string())?;

    let inferred = ((tile_width as f64) * (rh as f64) / (rw as f64)).round() as u32;
    Ok(inferred.max(2))
}

fn maybe_rotate(img: DynamicImage, target_w: u32, target_h: u32, auto_rotate: i32) -> DynamicImage {
    if auto_rotate == 0 {
        return img;
    }
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img;
    }
    let ratio = target_w as f64 / target_h as f64;
    let cur_diff = (w as f64 / h as f64 - ratio).abs();
    let rot_diff = (h as f64 / w as f64 - ratio).abs();
    if rot_diff < cur_diff {
        if auto_rotate > 0 {
            // Counter-clockwise 90 degrees (matches np.rot90 k=1)
            img.rotate270()
        } else {
            // Clockwise 90 degrees (matches np.rot90 k=-1)
            img.rotate90()
        }
    } else {
        img
    }
}

fn resize_center(img: &DynamicImage, tw: u32, th: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let target_ratio = tw as f64 / th as f64;
    let src_ratio = w as f64 / h as f64;

    let (crop_x, crop_y, crop_w, crop_h) = if src_ratio > target_ratio {
        // Source is wider than target -> crop width
        let cw = ((h as f64) * target_ratio).round().clamp(1.0, w as f64) as u32;
        let x = (w - cw) / 2;
        (x, 0, cw, h)
    } else {
        // Source is taller than target -> crop height
        let ch = ((w as f64) / target_ratio).round().clamp(1.0, h as f64) as u32;
        let y = (h - ch) / 2;
        (0, y, w, ch)
    };

    let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);
    cropped.resize_exact(tw, th, FilterType::Triangle).to_rgb8()
}

fn resize_fit(img: &DynamicImage, tw: u32, th: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let aspect = w as f64 / h as f64;
    let target_aspect = tw as f64 / th as f64;

    let (new_w, new_h) = if target_aspect > aspect {
        let nh = th;
        let nw = ((nh as f64) * aspect).round().clamp(1.0, tw as f64) as u32;
        (nw, nh)
    } else {
        let nw = tw;
        let nh = ((nw as f64) / aspect).round().clamp(1.0, th as f64) as u32;
        (nw, nh)
    };

    let scaled = img.resize_exact(new_w, new_h, FilterType::Triangle).to_rgb8();
    let mut canvas = RgbImage::from_pixel(tw, th, Rgb([255, 255, 255]));
    let off_x = (tw - new_w) / 2;
    let off_y = (th - new_h) / 2;

    for y in 0..new_h {
        for x in 0..new_w {
            canvas.put_pixel(off_x + x, off_y + y, *scaled.get_pixel(x, y));
        }
    }
    canvas
}

fn load_single_tile(
    path: &Path,
    tw: u32,
    th: u32,
    resize_opt: ResizeOption,
    auto_rotate: i32,
) -> Option<TileImage> {
    let img = image::open(path).ok()?;
    let img = maybe_rotate(img, tw, th, auto_rotate);
    let rgb_img = match resize_opt {
        ResizeOption::Center => resize_center(&img, tw, th),
        ResizeOption::Stretch => img.resize_exact(tw, th, FilterType::Triangle).to_rgb8(),
        ResizeOption::Fit => resize_fit(&img, tw, th),
    };

    Some(TileImage {
        width: tw,
        height: th,
        rgb: rgb_img.into_raw(),
        path: path.to_string_lossy().into_owned(),
    })
}

pub struct LoadedTiles {
    pub tiles: Vec<TileImage>,
    pub skipped: usize,
    pub tile_width: u32,
    pub tile_height: u32,
}

pub fn load_tiles_from_dir<F>(
    dir: &Path,
    tile_width: u32,
    tile_height: Option<u32>,
    infer_height: bool,
    resize_opt: ResizeOption,
    auto_rotate: i32,
    recursive: bool,
    on_progress: F,
) -> Result<LoadedTiles, String>
where
    F: Fn(usize, usize) + Sync,
{
    if tile_width < 2 {
        return Err("Tile width must be at least 2 pixels.".to_string());
    }

    let files = collect_files(dir, recursive)?;
    let th = if infer_height {
        infer_tile_height(&files, tile_width)?
    } else {
        let h = tile_height.unwrap_or(tile_width);
        if h < 2 {
            return Err("Tile height must be at least 2 pixels.".to_string());
        }
        h
    };

    let total = files.len();
    let counter = AtomicUsize::new(0);
    let step = (total / 40).max(1);

    let loaded: Vec<Option<TileImage>> = files
        .par_iter()
        .map(|path| {
            let res = load_single_tile(path, tile_width, th, resize_opt, auto_rotate);
            let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if done % step == 0 || done == total {
                on_progress(done, total);
            }
            res
        })
        .collect();

    let mut tiles = Vec::with_capacity(loaded.len());
    let mut skipped = 0usize;
    for item in loaded {
        if let Some(t) = item {
            tiles.push(t);
        } else {
            skipped += 1;
        }
    }

    if tiles.is_empty() {
        return Err("No decodable image files were found in the selected folder.".to_string());
    }

    Ok(LoadedTiles {
        tiles,
        skipped,
        tile_width,
        tile_height: th,
    })
}

