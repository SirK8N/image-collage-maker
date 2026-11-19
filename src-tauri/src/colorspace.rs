use crate::types::{ColorSpace, DistanceMetric, SortMethod, TileImage};
use rand::Rng;

#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn lab_f(t: f32) -> f32 {
    if t > 0.008856 {
        t.cbrt()
    } else {
        7.787 * t + 16.0 / 116.0
    }
}

#[inline]
pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta <= 1e-7 {
        0.0
    } else if (max - r).abs() <= 1e-7 {
        let mut h = ((g - b) / delta) % 6.0;
        if h < 0.0 {
            h += 6.0;
        }
        h / 6.0
    } else if (max - g).abs() <= 1e-7 {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    let s = if max <= 1e-7 { 0.0 } else { delta / max };
    [h, s, max]
}

#[inline]
pub fn rgb_to_hls(r: f32, g: f32, b: f32) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let l = 0.5 * (max + min);

    let s = if delta <= 1e-7 {
        0.0
    } else if l <= 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };

    let h = if delta <= 1e-7 {
        0.0
    } else if (max - r).abs() <= 1e-7 {
        let mut h = ((g - b) / delta) % 6.0;
        if h < 0.0 {
            h += 6.0;
        }
        h / 6.0
    } else if (max - g).abs() <= 1e-7 {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    [h, l, s]
}

#[inline]
fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 0.5 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

#[inline]
pub fn hls_to_rgb(h: f32, l: f32, s: f32) -> [f32; 3] {
    if s <= 1e-7 {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    [
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    ]
}

#[inline]
pub fn rgb_to_lab(r: f32, g: f32, b: f32) -> [f32; 3] {
    let rl = srgb_to_linear(r);
    let gl = srgb_to_linear(g);
    let bl = srgb_to_linear(b);

    let x = (0.412453 * rl + 0.357580 * gl + 0.180423 * bl) / 0.950456;
    let y = 0.212671 * rl + 0.715160 * gl + 0.072169 * bl;
    let z = (0.019334 * rl + 0.119193 * gl + 0.950227 * bl) / 1.088754;

    let fx = lab_f(x);
    let fy = lab_f(y);
    let fz = lab_f(z);

    let l = if y > 0.008856 {
        116.0 * fy - 16.0
    } else {
        903.3 * y
    };
    let a = 500.0 * (fx - fy);
    let bb = 200.0 * (fy - fz);
    [l, a, bb]
}

#[inline]
pub fn rgb_to_luv(r: f32, g: f32, b: f32) -> [f32; 3] {
    let rl = srgb_to_linear(r);
    let gl = srgb_to_linear(g);
    let bl = srgb_to_linear(b);

    let x = 0.412453 * rl + 0.357580 * gl + 0.180423 * bl;
    let y = 0.212671 * rl + 0.715160 * gl + 0.072169 * bl;
    let z = 0.019334 * rl + 0.119193 * gl + 0.950227 * bl;

    let l = if y > 0.008856 {
        116.0 * y.cbrt() - 16.0
    } else {
        903.3 * y
    };

    let denom = x + 15.0 * y + 3.0 * z;
    if denom <= 1e-6 {
        return [l, 0.0, 0.0];
    }

    let u_prime = 4.0 * x / denom;
    let v_prime = 9.0 * y / denom;
    let u = 13.0 * l * (u_prime - 0.19793943);
    let v = 13.0 * l * (v_prime - 0.46831096);
    [l, u, v]
}

/// Convert a slice of RGB u8 pixels (length 3*N) into float32 features in the requested colorspace.
pub fn convert_rgb_buffer(rgb: &[u8], colorspace: ColorSpace) -> Vec<f32> {
    let mut out = vec![0.0f32; rgb.len()];
    const INV_255: f32 = 1.0 / 255.0;

    match colorspace {
        ColorSpace::Bgr => {
            for (dst, src) in out.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
                dst[0] = src[2] as f32 * INV_255;
                dst[1] = src[1] as f32 * INV_255;
                dst[2] = src[0] as f32 * INV_255;
            }
        }
        ColorSpace::Hsv => {
            for (dst, src) in out.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
                let c = rgb_to_hsv(
                    src[0] as f32 * INV_255,
                    src[1] as f32 * INV_255,
                    src[2] as f32 * INV_255,
                );
                dst[0] = c[0];
                dst[1] = c[1];
                dst[2] = c[2];
            }
        }
        ColorSpace::Hsl => {
            for (dst, src) in out.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
                let c = rgb_to_hls(
                    src[0] as f32 * INV_255,
                    src[1] as f32 * INV_255,
                    src[2] as f32 * INV_255,
                );
                dst[0] = c[0];
                dst[1] = c[1];
                dst[2] = c[2];
            }
        }
        ColorSpace::Lab => {
            for (dst, src) in out.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
                let c = rgb_to_lab(
                    src[0] as f32 * INV_255,
                    src[1] as f32 * INV_255,
                    src[2] as f32 * INV_255,
                );
                dst[0] = c[0];
                dst[1] = c[1];
                dst[2] = c[2];
            }
        }
        ColorSpace::Luv => {
            for (dst, src) in out.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
                let c = rgb_to_luv(
                    src[0] as f32 * INV_255,
                    src[1] as f32 * INV_255,
                    src[2] as f32 * INV_255,
                );
                dst[0] = c[0];
                dst[1] = c[1];
                dst[2] = c[2];
            }
        }
    }

    out
}

/// Precomputed feature matrix for fast pairwise distance calculations.
pub struct FeatureIndex {
    pub metric: DistanceMetric,
    pub dim: usize,
    pub count: usize,
    /// Flat row-major vectors: count * dim
    pub data: Vec<f32>,
    /// Precomputed squared L2 norms (for Euclidean) or normalized vectors (for Cosine)
    pub aux: Vec<f32>,
}

impl FeatureIndex {
    pub fn new(metric: DistanceMetric, data: Vec<f32>, dim: usize) -> Self {
        let count = if dim > 0 { data.len() / dim } else { 0 };
        let aux = match metric {
            DistanceMetric::Euclidean => {
                let mut sq = Vec::with_capacity(count);
                for row in data.chunks_exact(dim) {
                    let s: f32 = row.iter().map(|&v| v * v).sum();
                    sq.push(s);
                }
                sq
            }
            DistanceMetric::Cosine => {
                let mut normed = vec![0.0f32; data.len()];
                for (dst, src) in normed.chunks_exact_mut(dim).zip(data.chunks_exact(dim)) {
                    let norm = src.iter().map(|&v| v * v).sum::<f32>().sqrt();
                    if norm > 1e-8 {
                        let inv = 1.0 / norm;
                        for (d, &s) in dst.iter_mut().zip(src.iter()) {
                            *d = s * inv;
                        }
                    }
                }
                normed
            }
            _ => Vec::new(),
        };

        Self {
            metric,
            dim,
            count,
            data,
            aux,
        }
    }

    /// Compute distance from query vector `q` (len == dim) to every tile vector, writing into `out` (len == count).
    pub fn distances_to(&self, q: &[f32], out: &mut [f32]) {
        debug_assert_eq!(q.len(), self.dim);
        debug_assert_eq!(out.len(), self.count);

        match self.metric {
            DistanceMetric::Euclidean => {
                let q_sq: f32 = q.iter().map(|&v| v * v).sum();
                for (i, (row, &b_sq)) in self
                    .data
                    .chunks_exact(self.dim)
                    .zip(self.aux.iter())
                    .enumerate()
                {
                    let mut dot = 0.0f32;
                    for (&a, &b) in q.iter().zip(row.iter()) {
                        dot += a * b;
                    }
                    // Guard against small negative floating-point cancellation
                    out[i] = (q_sq + b_sq - 2.0 * dot).max(0.0);
                }
            }
            DistanceMetric::Cityblock => {
                for (i, row) in self.data.chunks_exact(self.dim).enumerate() {
                    let mut sum = 0.0f32;
                    for (&a, &b) in q.iter().zip(row.iter()) {
                        sum += (a - b).abs();
                    }
                    out[i] = sum;
                }
            }
            DistanceMetric::Chebyshev => {
                for (i, row) in self.data.chunks_exact(self.dim).enumerate() {
                    let mut max_d = 0.0f32;
                    for (&a, &b) in q.iter().zip(row.iter()) {
                        let d = (a - b).abs();
                        if d > max_d {
                            max_d = d;
                        }
                    }
                    out[i] = max_d;
                }
            }
            DistanceMetric::Cosine => {
                let q_norm = q.iter().map(|&v| v * v).sum::<f32>().sqrt();
                if q_norm <= 1e-8 {
                    out.fill(1.0);
                    return;
                }
                let inv_q = 1.0 / q_norm;
                for (i, row) in self.aux.chunks_exact(self.dim).enumerate() {
                    let mut dot = 0.0f32;
                    for (&a, &b) in q.iter().zip(row.iter()) {
                        dot += a * b;
                    }
                    out[i] = 1.0 - dot * inv_q;
                }
            }
        }
    }

    pub fn tile_vec(&self, idx: usize) -> &[f32] {
        let start = idx * self.dim;
        &self.data[start..start + self.dim]
    }
}

/// Compute scalar sort key for a tile according to the selected sort method.
pub fn compute_sort_key(tile: &TileImage, method: SortMethod) -> f64 {
    let n_pixels = (tile.width as usize * tile.height as usize).max(1) as f64;
    match method {
        SortMethod::None => 0.0,
        SortMethod::BgrSum => tile.rgb.iter().map(|&v| v as f64).sum(),
        SortMethod::AvHue => {
            let mut sum = 0.0f64;
            for px in tile.rgb.chunks_exact(3) {
                let [h, _, _] = rgb_to_hsv(
                    px[0] as f32 / 255.0,
                    px[1] as f32 / 255.0,
                    px[2] as f32 / 255.0,
                );
                sum += h as f64;
            }
            sum / n_pixels
        }
        SortMethod::AvSat => {
            let mut sum = 0.0f64;
            for px in tile.rgb.chunks_exact(3) {
                let [_, s, _] = rgb_to_hsv(
                    px[0] as f32 / 255.0,
                    px[1] as f32 / 255.0,
                    px[2] as f32 / 255.0,
                );
                sum += s as f64;
            }
            sum / n_pixels
        }
        SortMethod::AvLum => {
            // Matching make_img.py's lum_coeffs = [0.241, 0.691, 0.068] on BGR
            let mut sum = 0.0f64;
            for px in tile.rgb.chunks_exact(3) {
                let r = px[0] as f64;
                let g = px[1] as f64;
                let b = px[2] as f64;
                sum += ((0.241 * b).sqrt() + (0.691 * g).sqrt() + (0.068 * r).sqrt()) / 3.0;
            }
            sum / n_pixels
        }
        SortMethod::Rand => rand::thread_rng().gen::<f64>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hls_roundtrip() {
        let samples = [
            [1.0f32, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.25, 0.6, 0.85],
            [0.5, 0.5, 0.5],
        ];
        for [r, g, b] in samples {
            let [h, l, s] = rgb_to_hls(r, g, b);
            let [r2, g2, b2] = hls_to_rgb(h, l, s);
            assert!((r - r2).abs() < 1e-5);
            assert!((g - g2).abs() < 1e-5);
            assert!((b - b2).abs() < 1e-5);
        }
    }

    #[test]
    fn test_lab_white_black() {
        let black = rgb_to_lab(0.0, 0.0, 0.0);
        assert!(black[0].abs() < 1e-3);
        let white = rgb_to_lab(1.0, 1.0, 1.0);
        assert!((white[0] - 100.0).abs() < 0.1);
        assert!(white[1].abs() < 0.1);
        assert!(white[2].abs() < 0.1);
    }
}

