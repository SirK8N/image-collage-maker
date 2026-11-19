/// Pure-Rust implementation of Montabone & Soto (2010) Fine-Grained Static Saliency
/// (equivalent to OpenCV's `cv::saliency::StaticSaliencyFineGrained`).
///
/// Uses an integral image (summed-area table) to compute center-surround ON/OFF
/// intensity differences across multiple spatial scales in O(W * H) time.
pub fn compute_fine_grained_saliency(rgb: &[u8], width: u32, height: u32) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 {
        return Vec::new();
    }

    // 1. Convert RGB to grayscale intensity [0, 255]
    let mut gray = vec![0.0f32; w * h];
    for (i, px) in rgb.chunks_exact(3).enumerate() {
        gray[i] = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
    }

    // 2. Build (h + 1) x (w + 1) integral image in f64 for exactness
    let iw = w + 1;
    let mut integral = vec![0.0f64; (h + 1) * iw];
    for y in 0..h {
        let mut row_sum = 0.0f64;
        let dst_row = (y + 1) * iw;
        let prev_row = y * iw;
        let src_row = y * w;
        for x in 0..w {
            row_sum += gray[src_row + x] as f64;
            integral[dst_row + x + 1] = integral[prev_row + x + 1] + row_sum;
        }
    }

    // Helper: mean intensity in clamped box [x0..=x1] x [y0..=y1]
    let box_mean = |x0: isize, y0: isize, x1: isize, y1: isize| -> f32 {
        let cx0 = x0.clamp(0, (w - 1) as isize) as usize;
        let cy0 = y0.clamp(0, (h - 1) as isize) as usize;
        let cx1 = x1.clamp(0, (w - 1) as isize) as usize + 1;
        let cy1 = y1.clamp(0, (h - 1) as isize) as usize + 1;
        let area = ((cx1 - cx0) * (cy1 - cy0)) as f64;
        let sum = integral[cy1 * iw + cx1]
            - integral[cy0 * iw + cx1]
            - integral[cy1 * iw + cx0]
            + integral[cy0 * iw + cx0];
        (sum / area) as f32
    };

    // 3. Multi-scale center-surround ON & OFF submaps (6 surround radii matching Montabone & Soto)
    let surround_radii: [isize; 6] = [3, 7, 13, 25, 49, 97];
    let mut saliency = vec![0.0f32; w * h];

    for &r in &surround_radii {
        let mut on_map = vec![0.0f32; w * h];
        let mut off_map = vec![0.0f32; w * h];
        let mut on_max = 0.0f32;
        let mut off_max = 0.0f32;

        for y in 0..h {
            let yi = y as isize;
            let row_off = y * w;
            for x in 0..w {
                let xi = x as isize;
                let center = gray[row_off + x];
                let surround = box_mean(xi - r, yi - r, xi + r, yi + r);
                let on = (center - surround).max(0.0);
                let off = (surround - center).max(0.0);
                on_map[row_off + x] = on;
                off_map[row_off + x] = off;
                if on > on_max {
                    on_max = on;
                }
                if off > off_max {
                    off_max = off;
                }
            }
        }

        // Normalize each submap before accumulating (gives balanced multi-scale response)
        let on_scale = if on_max > 1e-6 { 1.0 / on_max } else { 0.0 };
        let off_scale = if off_max > 1e-6 { 1.0 / off_max } else { 0.0 };
        for i in 0..(w * h) {
            saliency[i] += on_map[i] * on_scale + off_map[i] * off_scale;
        }
    }

    // 4. Normalize combined saliency map to [0.0, 1.0]
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for &v in &saliency {
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
    }

    let range = max_v - min_v;
    if range > 1e-6 {
        let inv = 1.0 / range;
        for v in &mut saliency {
            *v = (*v - min_v) * inv;
        }
    } else {
        saliency.fill(0.0);
    }

    saliency
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saliency_highlights_bright_center() {
        let w = 32;
        let h = 32;
        let mut rgb = vec![20u8; w * h * 3];
        // Bright square in the center
        for y in 12..20 {
            for x in 12..20 {
                let idx = (y * w + x) * 3;
                rgb[idx] = 250;
                rgb[idx + 1] = 250;
                rgb[idx + 2] = 250;
            }
        }
        let map = compute_fine_grained_saliency(&rgb, w as u32, h as u32);
        let center_val = map[16 * w + 16];
        let corner_val = map[0];
        assert!(center_val > corner_val + 0.3, "center={center_val}, corner={corner_val}");
    }
}

