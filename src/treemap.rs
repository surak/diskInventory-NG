//! Squarified treemap layout with cushion shading, after the KDirStat
//! algorithm (van Wijk & van de Wetering, "Cushion Treemaps").

use std::collections::HashMap;

use crate::scan::{Tree, NO_KIND};

/// `[x0, y0, x1, y1]` in image pixels.
pub type Rect = [f32; 4];

/// Initial cushion height and per-level scale factor.
const HEIGHT: f32 = 0.38;
const SCALE: f32 = 0.91;
/// Ambient and spot light intensity.
const AMBIENT: f32 = 0.20;
const SPOT: f32 = 0.85;

pub struct Layout {
    pub rects: HashMap<usize, Rect>,
    /// Painted leaf rectangles, for hit-testing.
    pub leaves: Vec<(Rect, usize)>,
}

impl Layout {
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        self.leaves
            .iter()
            .find(|(r, _)| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
            .map(|&(_, i)| i)
    }
}

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

pub fn render(tree: &Tree, root: usize, width: usize, height: usize) -> (Image, Layout) {
    let mut r = Renderer {
        tree,
        img: Image { width, height, rgba: vec![0; width * height * 4] },
        layout: Layout { rects: HashMap::new(), leaves: Vec::new() },
        light: {
            let (x, y, z) = (-1.0f32, -1.0f32, 10.0f32);
            let len = (x * x + y * y + z * z).sqrt();
            [x / len, y / len, z / len]
        },
    };
    r.node(root, [0.0, 0.0, width as f32, height as f32], [0.0; 4], HEIGHT);
    (r.img, r.layout)
}

struct Renderer<'a> {
    tree: &'a Tree,
    img: Image,
    layout: Layout,
    light: [f32; 3],
}

impl Renderer<'_> {
    fn node(&mut self, idx: usize, rect: Rect, surface: [f32; 4], h: f32) {
        if rect[2] - rect[0] < 0.5 || rect[3] - rect[1] < 0.5 {
            return;
        }
        self.layout.rects.insert(idx, rect);

        let [mut a, mut b, mut c, mut d] = surface;
        add_ridge(rect[0], rect[2], h, &mut a, &mut c);
        add_ridge(rect[1], rect[3], h, &mut b, &mut d);
        let s = [a, b, c, d];

        let n = &self.tree.nodes[idx];
        let children: Vec<(usize, u64)> = n
            .children
            .iter()
            .map(|&c| (c, self.tree.nodes[c].size))
            .filter(|&(_, size)| size > 0)
            .collect();

        if children.is_empty() {
            let color = match n.kind {
                NO_KIND => [110, 110, 110],
                k => self.tree.kinds[k as usize].color,
            };
            self.paint(rect, &s, color);
            self.layout.leaves.push((rect, idx));
            return;
        }

        for (child, r) in squarify(&children, rect) {
            self.node(child, r, s, h * SCALE);
        }
    }

    fn paint(&mut self, rect: Rect, s: &[f32; 4], color: [u8; 3]) {
        let (w, h) = (self.img.width, self.img.height);
        let x0 = (rect[0].round() as usize).min(w);
        let x1 = (rect[2].round() as usize).min(w);
        let y0 = (rect[1].round() as usize).min(h);
        let y1 = (rect[3].round() as usize).min(h);
        let [lx, ly, lz] = self.light;

        for y in y0..y1 {
            let ny = -(2.0 * s[1] * (y as f32 + 0.5) + s[3]);
            let row = &mut self.img.rgba[(y * w + x0) * 4..(y * w + x1) * 4];
            for (i, px) in row.chunks_exact_mut(4).enumerate() {
                let x = (x0 + i) as f32 + 0.5;
                let nx = -(2.0 * s[0] * x + s[2]);
                let cosa = (nx * lx + ny * ly + lz) / (nx * nx + ny * ny + 1.0).sqrt();
                let intensity = AMBIENT + SPOT * cosa.max(0.0);
                for c in 0..3 {
                    px[c] = (color[c] as f32 * intensity).min(255.0) as u8;
                }
                px[3] = 255;
            }
        }
    }
}

fn add_ridge(x1: f32, x2: f32, h: f32, s1: &mut f32, s2: &mut f32) {
    let w = x2 - x1;
    if w > 0.0 {
        *s2 += 4.0 * h * (x2 + x1) / w;
        *s1 -= 4.0 * h / w;
    }
}

/// Squarified layout (Bruls, Huizing, van Wijk). `items` must be sorted by
/// size, largest first, and contain no zero sizes.
pub fn squarify(items: &[(usize, u64)], rect: Rect) -> Vec<(usize, Rect)> {
    let mut out = Vec::with_capacity(items.len());
    let [mut x0, mut y0, x1, y1] = rect;
    let mut remaining: f64 = items.iter().map(|&(_, s)| s as f64).sum();
    let mut i = 0;

    while i < items.len() {
        let (w, h) = ((x1 - x0) as f64, (y1 - y0) as f64);
        if w <= 0.0 || h <= 0.0 || remaining <= 0.0 {
            break;
        }
        let scale = w * h / remaining;
        let side = w.min(h);

        // Grow the row while the worst aspect ratio improves.
        let mut sum = 0.0;
        let mut end = i;
        let mut best = f64::INFINITY;
        while end < items.len() {
            let a = items[end].1 as f64 * scale;
            let new_sum = sum + a;
            let largest = items[i].1 as f64 * scale;
            let worst = (side * side * largest / (new_sum * new_sum))
                .max(new_sum * new_sum / (side * side * a));
            if worst > best {
                break;
            }
            best = worst;
            sum = new_sum;
            end += 1;
        }

        let last = end == items.len();
        let row_raw: f64 = items[i..end].iter().map(|&(_, s)| s as f64).sum();
        let thickness = if last { if w >= h { w } else { h } } else { sum / side };

        // Lay the row along the shorter side.
        let mut pos = 0.0;
        for &(idx, size) in &items[i..end] {
            let len = side * size as f64 / row_raw;
            let r = if w >= h {
                [x0, y0 + pos as f32, x0 + thickness as f32, y0 + (pos + len) as f32]
            } else {
                [x0 + pos as f32, y0, x0 + (pos + len) as f32, y0 + thickness as f32]
            };
            out.push((idx, r));
            pos += len;
        }

        if w >= h {
            x0 += thickness as f32;
        } else {
            y0 += thickness as f32;
        }
        remaining -= row_raw;
        i = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_preserves_area() {
        let items = [(0, 600), (1, 300), (2, 60), (3, 30), (4, 10)];
        let out = squarify(&items, [0.0, 0.0, 100.0, 50.0]);
        assert_eq!(out.len(), items.len());
        for ((_, size), (_, r)) in items.iter().zip(&out) {
            let area = (r[2] - r[0]) * (r[3] - r[1]);
            let expected = *size as f32 / 1000.0 * 5000.0;
            assert!((area - expected).abs() < 0.5, "{area} vs {expected}");
            assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= 100.01 && r[3] <= 50.01);
        }
    }
}
