//! Procedurally drawn application icon: an isometric, extruded cushion
//! treemap — a nod to the 3D treemap icon of Disk Inventory X.

use crate::scan::{KindStat, Node, Tree, NO_KIND};
use crate::treemap;

/// A small made-up directory tree: (folder, file sizes with kind).
const SAMPLE: &[&[(u64, u32)]] = &[
    &[(420, 0), (180, 0), (90, 3), (60, 0)],
    &[(260, 1), (140, 1), (70, 4), (40, 1)],
    &[(200, 2), (110, 5), (50, 2)],
    &[(120, 3), (60, 4), (35, 5)],
];

fn sample_tree() -> Tree {
    let kinds = (0..6)
        .map(|k| KindStat {
            name: String::new(),
            size: 0,
            count: 0,
            color: crate::kinds::color_for_rank(k),
        })
        .collect();
    let node = |parent, size, is_dir, kind| Node {
        name: String::new(),
        parent,
        children: Vec::new(),
        size,
        is_dir,
        kind,
        files: 0,
        mtime: 0,
    };
    let mut nodes = vec![node(None, 0, true, NO_KIND)];
    for files in SAMPLE {
        let dir = nodes.len();
        nodes.push(node(Some(0), 0, true, NO_KIND));
        nodes[0].children.push(dir);
        for &(size, kind) in *files {
            let f = nodes.len();
            nodes.push(node(Some(dir), size, false, kind));
            nodes[dir].children.push(f);
            nodes[dir].size += size;
        }
        nodes[0].size += nodes[dir].size;
    }
    let mut dirs = nodes[0].children.clone();
    dirs.sort_by(|&a, &b| nodes[b].size.cmp(&nodes[a].size));
    nodes[0].children = dirs;
    Tree { nodes, kinds }
}

type P = (f32, f32);

fn fill_convex(img: &mut [[f32; 4]], size: usize, poly: &[P], mut shade: impl FnMut(f32, f32) -> Option<[f32; 3]>) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in poly {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let clamp = |v: f32| (v.max(0.0) as usize).min(size);
    for py in clamp(y0)..clamp(y1 + 1.0) {
        for px in clamp(x0)..clamp(x1 + 1.0) {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            // Inside test for a convex polygon of either winding.
            let mut sign = 0.0f32;
            let inside = poly.iter().zip(poly.iter().cycle().skip(1)).all(|(a, b)| {
                let c = (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
                if c.abs() < 1e-6 {
                    return true;
                }
                if sign == 0.0 {
                    sign = c.signum();
                }
                c.signum() == sign
            });
            if inside {
                if let Some(c) = shade(x, y) {
                    img[py * size + px] = [c[0], c[1], c[2], 1.0];
                }
            }
        }
    }
}

/// Renders the icon as straight RGBA at `size`×`size` pixels.
pub fn render(size: usize) -> Vec<u8> {
    const SS: usize = 3; // supersampling factor
    let big = size * SS;
    let tree = sample_tree();

    // Cushion texture for the block tops.
    const TEX: usize = 512;
    let (tex, layout) = treemap::render(&tree, 0, TEX, TEX);

    // Isometric projection of the unit square, with room for the extrusion.
    let s = big as f32 * 0.47;
    let (cx, cy) = (big as f32 * 0.5, big as f32 * 0.335);
    let (cos, sin) = (30f32.to_radians().cos(), 30f32.to_radians().sin());
    let project = |x: f32, y: f32, z: f32| -> P { (cx + (x - y) * cos * s, cy + (x + y) * sin * s - z * s) };

    // Leaves as boxes, back to front.
    let mut blocks: Vec<([f32; 4], usize)> = layout
        .leaves
        .iter()
        .map(|&(r, i)| (r.map(|v| v / TEX as f32), i))
        .collect();
    blocks.sort_by(|a, b| (a.0[2] + a.0[3]).total_cmp(&(b.0[2] + b.0[3])));

    let mut img = vec![[0f32; 4]; big * big];
    for (n, &(r, idx)) in blocks.iter().enumerate() {
        let [x0, y0, x1, y1] = r;
        let z = 0.10 + 0.05 * ((n * 7919 + idx * 31) % 5) as f32;
        let color = tree.kinds[tree.nodes[idx].kind as usize].color.map(|c| c as f32 / 255.0);
        let dark = |f: f32| Some(color.map(|c| c * f));

        // The two visible sides: x = x1 (right) and y = y1 (left).
        let right = [project(x1, y0, 0.0), project(x1, y1, 0.0), project(x1, y1, z), project(x1, y0, z)];
        fill_convex(&mut img, big, &right, |_, _| dark(0.55));
        let left = [project(x0, y1, 0.0), project(x1, y1, 0.0), project(x1, y1, z), project(x0, y1, z)];
        fill_convex(&mut img, big, &left, |_, _| dark(0.75));

        // Top face, textured with the cushion image via the inverse projection.
        let top = [project(x0, y0, z), project(x1, y0, z), project(x1, y1, z), project(x0, y1, z)];
        fill_convex(&mut img, big, &top, |sx, sy| {
            let a = (sx - cx) / (cos * s);
            let b = (sy - cy + z * s) / (sin * s);
            let (x, y) = ((a + b) / 2.0, (b - a) / 2.0);
            let tx = ((x * TEX as f32) as usize).min(TEX - 1);
            let ty = ((y * TEX as f32) as usize).min(TEX - 1);
            let p = &tex.rgba[(ty * TEX + tx) * 4..][..3];
            Some([p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
        });
    }

    // Downsample (box filter) with premultiplied alpha.
    let mut out = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for dy in 0..SS {
                for dx in 0..SS {
                    let p = img[(y * SS + dy) * big + x * SS + dx];
                    for c in 0..3 {
                        acc[c] += p[c] * p[3];
                    }
                    acc[3] += p[3];
                }
            }
            let o = &mut out[(y * size + x) * 4..][..4];
            if acc[3] > 0.0 {
                for c in 0..3 {
                    o[c] = (acc[c] / acc[3] * 255.0).round() as u8;
                }
            }
            o[3] = (acc[3] / (SS * SS) as f32 * 255.0).round() as u8;
        }
    }
    out
}

pub fn write_png(path: &std::path::Path, size: usize) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, size as u32, size as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&render(size))?;
    Ok(())
}
