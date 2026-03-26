//! Dual-band thermal image frame with tiling support.

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::tile::OverlapTile;
use crate::tile::Tile;
use crate::tile::compute_tiles_with_overlap;
use crate::tile::extract_tile;

/// A dual-band thermal image frame.
pub struct Frame<'a> {
    /// MWIR brightness temperatures (Kelvin).
    pub mwir: &'a [f32],
    /// LWIR brightness temperatures (Kelvin).
    pub lwir: &'a [f32],
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// Wire-format tile header (12 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct TileHeader {
    pub frame_x: u16,
    pub frame_y: u16,
    pub width: u16,
    pub height: u16,
    pub inner_w: u16,
    pub inner_h: u16,
}

/// A tile with extracted dual-band pixel data.
pub struct DualBandTile<'a> {
    pub geo: &'a OverlapTile,
    pub mwir: &'a [f32],
    pub lwir: &'a [f32],
}

impl DualBandTile<'_> {
    /// Serializes header + MWIR + LWIR into `buf`.
    /// Returns the number of bytes written.
    pub fn write_to(&self, buf: &mut [u8]) -> usize {
        let header = TileHeader {
            frame_x: self.geo.frame_x as u16,
            frame_y: self.geo.frame_y as u16,
            width: self.geo.width as u16,
            height: self.geo.height as u16,
            inner_w: self.geo.inner_w as u16,
            inner_h: self.geo.inner_h as u16,
        };
        let hdr = header.as_bytes();
        let mwir = self.mwir.as_bytes();
        let lwir = self.lwir.as_bytes();

        let mut off = 0;
        buf[off..off + hdr.len()].copy_from_slice(hdr);
        off += hdr.len();
        buf[off..off + mwir.len()].copy_from_slice(mwir);
        off += mwir.len();
        buf[off..off + lwir.len()].copy_from_slice(lwir);
        off += lwir.len();
        off
    }
}

/// A parsed tile received from a remote collector.
pub struct ReceivedTile<'a> {
    pub header: TileHeader,
    pub mwir: &'a [f32],
    pub lwir: &'a [f32],
}

impl<'a> ReceivedTile<'a> {
    /// Parses a tile from a byte buffer (header + MWIR + LWIR).
    pub fn from_bytes(data: &'a [u8]) -> Option<Self> {
        let (header, _) = TileHeader::read_from_prefix(data).ok()?;
        let hdr_size = core::mem::size_of::<TileHeader>();
        let n = header.width as usize * header.height as usize;
        let pixel_bytes = n * 4;
        let mwir_start = hdr_size;
        let lwir_start = mwir_start + pixel_bytes;
        let mwir = <[f32]>::ref_from_bytes(&data[mwir_start..mwir_start + pixel_bytes]).ok()?;
        let lwir = <[f32]>::ref_from_bytes(&data[lwir_start..lwir_start + pixel_bytes]).ok()?;
        Some(Self { header, mwir, lwir })
    }

    /// Overlap offset X (pixels from tile edge to inner region).
    pub fn overlap_x(&self) -> u16 {
        if self.header.frame_x >= self.header.width - self.header.inner_w {
            (self.header.width - self.header.inner_w).min(self.header.frame_x)
        } else {
            self.header.frame_x
        }
    }

    /// Overlap offset Y.
    pub fn overlap_y(&self) -> u16 {
        if self.header.frame_y >= self.header.height - self.header.inner_h {
            (self.header.height - self.header.inner_h).min(self.header.frame_y)
        } else {
            self.header.frame_y
        }
    }
}

/// Lending iterator over tiles of a frame.
///
/// The caller owns the scratch buffers. Each call to
/// `next()` extracts pixel data into them and returns
/// a `DualBandTile` borrowing the buffers.
pub struct TileIter<'frame, 'buf> {
    frame: &'frame Frame<'frame>,
    tiles: [OverlapTile; 256],
    count: usize,
    index: usize,
    mwir: &'buf mut [f32],
    lwir: &'buf mut [f32],
}

impl<'frame, 'buf> leodos_utils::lending_iterator::LendingIterator for TileIter<'frame, 'buf> {
    type Item<'a> = DualBandTile<'a> where Self: 'a;

    fn next(&mut self) -> Option<DualBandTile<'_>> {
        if self.index >= self.count {
            return None;
        }
        let tile = &self.tiles[self.index];
        let geom = Tile {
            col: 0, row: 0,
            x: tile.x, y: tile.y,
            width: tile.width, height: tile.height,
        };
        let w = self.frame.width as usize;
        extract_tile(self.frame.mwir, w, &geom, self.mwir);
        extract_tile(self.frame.lwir, w, &geom, self.lwir);

        let npx = tile.width * tile.height;
        self.index += 1;
        Some(DualBandTile {
            geo: tile,
            mwir: &self.mwir[..npx],
            lwir: &self.lwir[..npx],
        })
    }
}

const DEFAULT_TILE: OverlapTile = OverlapTile {
    x: 0, y: 0, width: 0, height: 0,
    inner_x: 0, inner_y: 0, inner_w: 0, inner_h: 0,
    frame_x: 0, frame_y: 0,
};

impl<'a> Frame<'a> {
    /// Number of tiles for the given tile size.
    pub fn tile_count(&self, tile_size: usize) -> usize {
        let cols = (self.width as usize + tile_size - 1) / tile_size;
        let rows = (self.height as usize + tile_size - 1) / tile_size;
        cols * rows
    }

    /// Creates a tile iterator over this frame.
    ///
    /// The caller provides scratch buffers for pixel extraction.
    /// Each buffer must be at least `(tile_size + 2*overlap)^2` floats.
    pub fn tiles<'buf>(
        &'a self,
        tile_size: usize,
        overlap: usize,
        mwir_buf: &'buf mut [f32],
        lwir_buf: &'buf mut [f32],
    ) -> TileIter<'a, 'buf> {
        let mut tiles = [DEFAULT_TILE; 256];
        let count = compute_tiles_with_overlap(
            self.width as usize,
            self.height as usize,
            tile_size,
            overlap,
            &mut tiles,
        );
        TileIter {
            frame: self,
            tiles,
            count,
            index: 0,
            mwir: mwir_buf,
            lwir: lwir_buf,
        }
    }
}
