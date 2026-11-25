use image::imageops;
use image_dds::{ImageFormat, Mipmaps, Quality, SurfaceRgba8};

/// An encoded image as a DDS of GPU-compressed blocks for 3D previews: BC1 (BC3 when
/// it has alpha), at most `max` px on its longer side, with every mip level. The GPU
/// takes it as is, without decoding or building mips on the main thread, at a quarter
/// to an eighth of the memory. Flipped vertically, since compressed textures can't be
/// flipped on upload. `None` when the image can't be decoded or encoded.
#[must_use]
pub fn bc_dds(bytes: &[u8], max: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let scale = (f64::from(max) / f64::from(w.max(h))).min(1.0);
    // Blocks are 4×4 pixels; WebGL wants the top level to be whole blocks.
    let fit = |n: u32| (((f64::from(n) * scale) / 4.0).round() as u32).max(1) * 4;
    let (tw, th) = (fit(w), fit(h));
    let img = if (tw, th) == (w, h) {
        img
    } else {
        imageops::thumbnail(&img, tw, th)
    };
    let img = imageops::flip_vertical(&img);
    let alpha = img.pixels().any(|p| p[3] < 255);
    let format = if alpha {
        ImageFormat::BC3RgbaUnorm
    } else {
        ImageFormat::BC1RgbaUnorm
    };
    let surface = SurfaceRgba8::from_image(&img)
        .encode(format, Quality::Fast, Mipmaps::GeneratedAutomatic)
        .ok()?;
    Some(dds(
        surface.width,
        surface.height,
        surface.mipmaps,
        alpha,
        &surface.data,
    ))
}

/// A DDS file with the classic header (fourCC `DXT1` / `DXT5`), which three.js's
/// `DDSLoader` reads; the newer DX10 header it reads only for BC6H.
fn dds(width: u32, height: u32, mips: u32, alpha: bool, data: &[u8]) -> Vec<u8> {
    const CAPS: u32 = 0x1;
    const HEIGHT: u32 = 0x2;
    const WIDTH: u32 = 0x4;
    const PIXEL_FORMAT: u32 = 0x1000;
    const MIPMAP_COUNT: u32 = 0x2_0000;
    const LINEAR_SIZE: u32 = 0x8_0000;
    const FOURCC: u32 = 0x4;
    const CAPS_COMPLEX: u32 = 0x8;
    const CAPS_TEXTURE: u32 = 0x1000;
    const CAPS_MIPMAP: u32 = 0x40_0000;
    let block = if alpha { 16 } else { 8 };
    let mut h = [0u32; 32];
    h[0] = u32::from_le_bytes(*b"DDS ");
    h[1] = 124;
    h[2] = CAPS | HEIGHT | WIDTH | PIXEL_FORMAT | MIPMAP_COUNT | LINEAR_SIZE;
    h[3] = height;
    h[4] = width;
    h[5] = width.div_ceil(4) * height.div_ceil(4) * block;
    h[7] = mips;
    h[19] = 32;
    h[20] = FOURCC;
    h[21] = u32::from_le_bytes(if alpha { *b"DXT5" } else { *b"DXT1" });
    h[27] = CAPS_TEXTURE | CAPS_MIPMAP | CAPS_COMPLEX;
    let mut out = Vec::with_capacity(128 + data.len());
    for v in h {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(w: u32, h: u32, alpha: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([x as u8, y as u8, 0, alpha]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn word(dds: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(dds[i * 4..i * 4 + 4].try_into().unwrap())
    }

    #[test]
    fn compresses_to_whole_blocks_with_every_mip_level() {
        let dds = bc_dds(&png(2048, 1000, 255), 1024).unwrap();
        assert_eq!(&dds[0..4], b"DDS ");
        assert_eq!((word(&dds, 4), word(&dds, 3)), (1024, 500), "width, height");
        assert_eq!(&dds[84..88], b"DXT1");
        assert_eq!(word(&dds, 7), 11, "1024 down to 1");
        assert_eq!(word(&dds, 5), 256 * 125 * 8);
        let levels: u32 = (0..11)
            .map(|l| (1024u32 >> l).max(1).div_ceil(4) * (500u32 >> l).max(1).div_ceil(4) * 8)
            .sum();
        assert_eq!(dds.len(), 128 + levels as usize);

        let small = bc_dds(&png(30, 30, 128), 1024).unwrap();
        assert_eq!(&small[84..88], b"DXT5", "alpha keeps BC3");
        assert_eq!((word(&small, 4), word(&small, 3)), (32, 32));
        assert!(bc_dds(b"not an image", 1024).is_none());
    }
}
