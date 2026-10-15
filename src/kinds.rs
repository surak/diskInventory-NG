//! Colors for file kinds. Like Disk Inventory X / WinDirStat, the biggest
//! kinds get distinct colors and everything else is drawn in grey.

const PALETTE: &[[u8; 3]] = &[
    [0, 0, 255],
    [255, 0, 0],
    [0, 200, 0],
    [0, 220, 220],
    [255, 0, 255],
    [255, 220, 0],
    [100, 100, 255],
    [255, 120, 0],
    [130, 255, 130],
    [0, 130, 255],
    [180, 0, 255],
    [200, 140, 80],
    [255, 140, 200],
    [120, 200, 0],
    [0, 150, 130],
    [180, 180, 255],
    [150, 70, 70],
    [255, 255, 140],
];

pub const OTHER: [u8; 3] = [160, 160, 160];

pub fn color_for_rank(rank: usize) -> [u8; 3] {
    PALETTE.get(rank).copied().unwrap_or(OTHER)
}
