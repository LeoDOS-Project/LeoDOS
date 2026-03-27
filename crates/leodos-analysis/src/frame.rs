//! Dual-band thermal image frame with tiling support.

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::tile::OverlapTile;

/// A dual-band thermal image frame (raw pixels).
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

/// A geo-located dual-band frame.
pub struct GeoFrame<'a> {
    pub frame: Frame<'a>,
    pub nadir_lat: f32,
    pub nadir_lon: f32,
    pub gsd: f32,
    pub timestamp_s: f64,
}

/// Wire-format tile header.
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct TileHeader {
    pub frame_x: u16,
    pub frame_y: u16,
    pub width: u16,
    pub height: u16,
    pub inner_w: u16,
    pub inner_h: u16,
    pub nadir_lat: f32,
    pub nadir_lon: f32,
    pub gsd: f32,
    pub timestamp_s: f64,
}

/// A parsed tile received from a remote collector.
pub struct TileMessage<'a> {
    pub header: TileHeader,
    pub mwir: &'a [f32],
    pub lwir: &'a [f32],
}

/// A zero-copy view of a tile within a dual-band frame.
///
/// Owns the tile geometry; pixel data is accessed through
/// the frame reference. No intermediate buffers needed.
pub struct DualBandTile<'a> {
    pub geo: OverlapTile,
    mwir: &'a [f32],
    lwir: &'a [f32],
    frame_width: usize,
    pub nadir_lat: f32,
    pub nadir_lon: f32,
    pub gsd: f32,
    pub timestamp_s: f64,
}

impl DualBandTile<'_> {
    /// Iterator over MWIR pixel rows for this tile.
    pub fn mwir_rows(&self) -> impl Iterator<Item = &[f32]> {
        self.rows(self.mwir)
    }

    /// Iterator over LWIR pixel rows for this tile.
    pub fn lwir_rows(&self) -> impl Iterator<Item = &[f32]> {
        self.rows(self.lwir)
    }

    fn rows<'a>(&'a self, band: &'a [f32]) -> impl Iterator<Item = &'a [f32]> {
        let x = self.geo.x;
        let w = self.geo.width;
        let fw = self.frame_width;
        (0..self.geo.height).map(move |row| {
            let start = (self.geo.y + row) * fw + x;
            &band[start..start + w]
        })
    }

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
            nadir_lat: self.nadir_lat,
            nadir_lon: self.nadir_lon,
            gsd: self.gsd,
            timestamp_s: self.timestamp_s,
        };
        let hdr = header.as_bytes();
        let mut off = 0;
        buf[off..off + hdr.len()].copy_from_slice(hdr);
        off += hdr.len();

        for row in self.mwir_rows() {
            let bytes = row.as_bytes();
            buf[off..off + bytes.len()].copy_from_slice(bytes);
            off += bytes.len();
        }
        for row in self.lwir_rows() {
            let bytes = row.as_bytes();
            buf[off..off + bytes.len()].copy_from_slice(bytes);
            off += bytes.len();
        }
        off
    }
}

impl<'a> TileMessage<'a> {
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

    pub fn width(&self) -> usize {
        self.header.width as usize
    }

    pub fn height(&self) -> usize {
        self.header.height as usize
    }

    pub fn nadir_lat(&self) -> f32 {
        self.header.nadir_lat
    }

    pub fn nadir_lon(&self) -> f32 {
        self.header.nadir_lon
    }

    pub fn gsd(&self) -> f32 {
        self.header.gsd
    }

    pub fn timestamp_s(&self) -> f64 {
        self.header.timestamp_s
    }

    /// Maps tile-local coordinates to frame coordinates.
    ///
    /// Returns `None` if the point falls in the overlap border.
    pub fn to_frame_coords(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        let ox = self.overlap_x();
        let oy = self.overlap_y();
        (x >= ox && x < ox + self.header.inner_w && y >= oy && y < oy + self.header.inner_h).then(
            || {
                let fx = self.header.frame_x + (x - ox);
                let fy = self.header.frame_y + (y - oy);
                (fx, fy)
            },
        )
    }

    fn overlap_x(&self) -> u16 {
        if self.header.frame_x >= self.header.width - self.header.inner_w {
            (self.header.width - self.header.inner_w).min(self.header.frame_x)
        } else {
            self.header.frame_x
        }
    }

    fn overlap_y(&self) -> u16 {
        if self.header.frame_y >= self.header.height - self.header.inner_h {
            (self.header.height - self.header.inner_h).min(self.header.frame_y)
        } else {
            self.header.frame_y
        }
    }
}

/// Iterator over tiles of a frame. Computes tile geometry on the fly.
pub struct TileIter<'frame> {
    mwir: &'frame [f32],
    lwir: &'frame [f32],
    frame_width: usize,
    frame_height: usize,
    tile_size: usize,
    overlap: usize,
    nadir_lat: f32,
    nadir_lon: f32,
    gsd: f32,
    timestamp_s: f64,
    cols: usize,
    total: usize,
    index: usize,
}

impl<'frame> Iterator for TileIter<'frame> {
    type Item = DualBandTile<'frame>;

    fn next(&mut self) -> Option<DualBandTile<'frame>> {
        if self.index >= self.total {
            return None;
        }
        let col = self.index % self.cols;
        let row = self.index / self.cols;
        self.index += 1;

        let orig_x = col * self.tile_size;
        let orig_y = row * self.tile_size;
        let inner_w = self.tile_size.min(self.frame_width - orig_x);
        let inner_h = self.tile_size.min(self.frame_height - orig_y);
        let x0 = orig_x.saturating_sub(self.overlap);
        let y0 = orig_y.saturating_sub(self.overlap);
        let x1 = (orig_x + inner_w + self.overlap).min(self.frame_width);
        let y1 = (orig_y + inner_h + self.overlap).min(self.frame_height);

        Some(DualBandTile {
            geo: OverlapTile {
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
            },
            mwir: self.mwir,
            lwir: self.lwir,
            frame_width: self.frame_width,
            nadir_lat: self.nadir_lat,
            nadir_lon: self.nadir_lon,
            gsd: self.gsd,
            timestamp_s: self.timestamp_s,
        })
    }
}

impl<'a> Frame<'a> {
    /// Returns an iterator over tiles (without geo context).
    pub fn tiles(&'a self, tile_size: usize, overlap: usize) -> TileIter<'a> {
        let w = self.width as usize;
        let h = self.height as usize;
        let cols = (w + tile_size - 1) / tile_size;
        let rows = (h + tile_size - 1) / tile_size;
        TileIter {
            mwir: self.mwir,
            lwir: self.lwir,
            frame_width: w,
            frame_height: h,
            tile_size,
            overlap,
            nadir_lat: 0.0,
            nadir_lon: 0.0,
            gsd: 0.0,
            timestamp_s: 0.0,
            cols,
            total: cols * rows,
            index: 0,
        }
    }
}

impl<'a> GeoFrame<'a> {
    /// Returns an iterator over geo-located tiles.
    pub fn tiles(&'a self, tile_size: usize, overlap: usize) -> TileIter<'a> {
        let w = self.frame.width as usize;
        let h = self.frame.height as usize;
        let cols = (w + tile_size - 1) / tile_size;
        let rows = (h + tile_size - 1) / tile_size;
        TileIter {
            mwir: self.frame.mwir,
            lwir: self.frame.lwir,
            frame_width: w,
            frame_height: h,
            tile_size,
            overlap,
            nadir_lat: self.nadir_lat,
            nadir_lon: self.nadir_lon,
            gsd: self.gsd,
            timestamp_s: self.timestamp_s,
            cols,
            total: cols * rows,
            index: 0,
        }
    }
}
