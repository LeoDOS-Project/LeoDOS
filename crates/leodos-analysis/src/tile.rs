//! Image tiling for distributed processing.
//!
//! Splits a large swath into fixed-size tiles that can be
//! independently processed by SpaceCoMP map tasks. Handles
//! partial tiles at image boundaries.

/// A tile region within a larger image.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Tile {
    /// Tile column index.
    pub col: u16,
    /// Tile row index.
    pub row: u16,
    /// Pixel x offset within the image.
    pub x: usize,
    /// Pixel y offset within the image.
    pub y: usize,
    /// Tile width in pixels (may be < tile_size at right edge).
    pub width: usize,
    /// Tile height in pixels (may be < tile_size at bottom edge).
    pub height: usize,
}

/// Compute tile layout for an image.
///
/// Returns the number of tiles written to `tiles`.
pub fn compute_tiles(
    image_width: usize,
    image_height: usize,
    tile_size: usize,
    tiles: &mut [Tile],
) -> usize {
    let cols = (image_width + tile_size - 1) / tile_size;
    let rows = (image_height + tile_size - 1) / tile_size;
    let mut count = 0;

    for row in 0..rows {
        for col in 0..cols {
            if count >= tiles.len() {
                return count;
            }
            let x = col * tile_size;
            let y = row * tile_size;
            let w = tile_size.min(image_width - x);
            let h = tile_size.min(image_height - y);
            tiles[count] = Tile {
                col: col as u16,
                row: row as u16,
                x,
                y,
                width: w,
                height: h,
            };
            count += 1;
        }
    }
    count
}

/// Number of tiles for given image and tile dimensions.
pub fn tile_count(image_width: usize, image_height: usize, tile_size: usize) -> usize {
    let cols = (image_width + tile_size - 1) / tile_size;
    let rows = (image_height + tile_size - 1) / tile_size;
    cols * rows
}

/// Extract a tile's data from a flat image buffer into a tile buffer.
///
/// The image is stored row-major: `image[y * image_width + x]`.
/// The output is stored row-major within the tile.
pub fn extract_tile(
    image: &[f32],
    image_width: usize,
    tile: &Tile,
    output: &mut [f32],
) {
    for ty in 0..tile.height {
        let src_start = (tile.y + ty) * image_width + tile.x;
        let dst_start = ty * tile.width;
        output[dst_start..dst_start + tile.width]
            .copy_from_slice(&image[src_start..src_start + tile.width]);
    }
}

/// Computes tile layout with overlap border for contextual algorithms.
///
/// Each tile's region is expanded by `overlap` pixels in each direction
/// (clamped to image bounds). The `x`, `y`, `width`, `height` fields
/// reflect the expanded region. `inner_x`, `inner_y` mark the original
/// tile origin within the expanded region.
pub fn compute_tiles_with_overlap(
    image_width: usize,
    image_height: usize,
    tile_size: usize,
    overlap: usize,
    tiles: &mut [OverlapTile],
) -> usize {
    let cols = (image_width + tile_size - 1) / tile_size;
    let rows = (image_height + tile_size - 1) / tile_size;
    let mut count = 0;

    for row in 0..rows {
        for col in 0..cols {
            if count >= tiles.len() {
                return count;
            }
            let orig_x = col * tile_size;
            let orig_y = row * tile_size;
            let inner_w = tile_size.min(image_width - orig_x);
            let inner_h = tile_size.min(image_height - orig_y);

            let x0 = orig_x.saturating_sub(overlap);
            let y0 = orig_y.saturating_sub(overlap);
            let x1 = (orig_x + inner_w + overlap).min(image_width);
            let y1 = (orig_y + inner_h + overlap).min(image_height);

            tiles[count] = OverlapTile {
                x: x0,
                y: y0,
                width: x1 - x0,
                height: y1 - y0,
                inner_x: orig_x - x0,
                inner_y: orig_y - y0,
                inner_w,
                inner_h,
                frame_x: orig_x,
                frame_y: orig_y,
            };
            count += 1;
        }
    }
    count
}

/// A tile region with overlap border.
#[derive(Debug, Copy, Clone)]
pub struct OverlapTile {
    /// Top-left X of the expanded region.
    pub x: usize,
    /// Top-left Y of the expanded region.
    pub y: usize,
    /// Width of the expanded region (including overlap).
    pub width: usize,
    /// Height of the expanded region (including overlap).
    pub height: usize,
    /// X offset of the inner region within the expanded tile.
    pub inner_x: usize,
    /// Y offset of the inner region within the expanded tile.
    pub inner_y: usize,
    /// Width of the inner region (no overlap).
    pub inner_w: usize,
    /// Height of the inner region (no overlap).
    pub inner_h: usize,
    /// X position of the inner region in the full frame.
    pub frame_x: usize,
    /// Y position of the inner region in the full frame.
    pub frame_y: usize,
}

/// Write a tile's data back into a flat image buffer.
pub fn insert_tile(
    tile_data: &[f32],
    tile: &Tile,
    image: &mut [f32],
    image_width: usize,
) {
    for ty in 0..tile.height {
        let src_start = ty * tile.width;
        let dst_start = (tile.y + ty) * image_width + tile.x;
        image[dst_start..dst_start + tile.width]
            .copy_from_slice(&tile_data[src_start..src_start + tile.width]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_count_exact() {
        assert_eq!(tile_count(256, 256, 128), 4);
        assert_eq!(tile_count(256, 256, 256), 1);
    }

    #[test]
    fn tile_count_partial() {
        assert_eq!(tile_count(300, 200, 128), 6);
    }

    #[test]
    fn compute_tiles_layout() {
        let mut tiles = [Tile {
            col: 0, row: 0, x: 0, y: 0, width: 0, height: 0,
        }; 16];
        let n = compute_tiles(300, 200, 128, &mut tiles);
        assert_eq!(n, 6);

        assert_eq!(tiles[0].width, 128);
        assert_eq!(tiles[0].height, 128);

        assert_eq!(tiles[2].x, 256);
        assert_eq!(tiles[2].width, 44);
        assert_eq!(tiles[2].height, 128);

        assert_eq!(tiles[5].y, 128);
        assert_eq!(tiles[5].height, 72);
    }

    #[test]
    fn extract_insert_roundtrip() {
        let image: [f32; 16] = core::array::from_fn(|i| i as f32);
        let tile = Tile {
            col: 1, row: 0, x: 2, y: 0, width: 2, height: 2,
        };

        let mut tile_buf = [0.0f32; 4];
        extract_tile(&image, 4, &tile, &mut tile_buf);
        assert_eq!(tile_buf, [2.0, 3.0, 6.0, 7.0]);

        let mut out_image = [0.0f32; 16];
        insert_tile(&tile_buf, &tile, &mut out_image, 4);
        assert_eq!(out_image[2], 2.0);
        assert_eq!(out_image[3], 3.0);
        assert_eq!(out_image[6], 6.0);
        assert_eq!(out_image[7], 7.0);
        assert_eq!(out_image[0], 0.0);
    }
}
