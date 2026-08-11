//! Convert a premultiplied RGBA buffer into the top-down 32-bit DIB bitmap the
//! shell expects back from `IThumbnailProvider::GetThumbnail`.

use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Graphics::Gdi::{
    CreateDIBSection, DeleteObject, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP,
    RGBQUAD,
};
use windows_sys::Win32::UI::Shell::{WTS_ALPHATYPE, WTSAT_ARGB, WTSAT_RGB};

/// Build a device-independent bitmap from `premultiplied_rgba`.
///
/// The caller (the shell) owns the returned `HBITMAP` and is responsible for
/// releasing it with `DeleteObject`. Returns `Err(())` if the bitmap cannot be
/// created; the provider then reports `E_FAIL` and the shell falls back to the
/// default file icon.
pub(crate) fn rgba_to_hbitmap(
    width: u32,
    height: u32,
    premultiplied_rgba: &[u8],
) -> Result<(HBITMAP, WTS_ALPHATYPE), ()> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|area| area.checked_mul(4));
    if expected != Some(premultiplied_rgba.len()) {
        return Err(());
    }
    if width == 0 || height == 0 {
        return Err(());
    }

    let has_alpha = premultiplied_rgba.chunks_exact(4).any(|pixel| pixel[3] < 255);

    let bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // A negative height requests a top-down DIB, matching row order of
            // the raw RGBA buffer (first byte = top-left pixel).
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        bmiColors: [RGBQUAD {
            rgbBlue: 0,
            rgbGreen: 0,
            rgbRed: 0,
            rgbReserved: 0,
        }],
    };

    let mut bits: *mut core::ffi::c_void = ptr::null_mut();
    // SAFETY: `bitmap_info` describes a 32-bit top-down DIB with no external
    // palette, and `bits` receives the pixel buffer owned by the new section.
    let bitmap = unsafe {
        CreateDIBSection(
            ptr::null_mut(), // memory DIB, no device context needed
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            ptr::null_mut(), // no file mapping
            0,
        )
    };
    if bitmap.is_null() || bits.is_null() {
        if !bitmap.is_null() {
            // SAFETY: `bitmap` was created above and has not been handed out.
            unsafe { DeleteObject(bitmap as _) };
        }
        return Err(());
    }

    // The shell expects BGRA, so swap red and blue in place. Premultiplied
    // alpha is preserved as-is (tiny-skia already premultiplies).
    // SAFETY: `bits` points to `width * height * 4` bytes owned by the DIB
    // section for the lifetime of `bitmap`.
    unsafe {
        let destination =
            std::slice::from_raw_parts_mut(bits as *mut u8, premultiplied_rgba.len());
        for (dst, src) in destination
            .chunks_exact_mut(4)
            .zip(premultiplied_rgba.chunks_exact(4))
        {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
    }

    let alpha_type = if has_alpha { WTSAT_ARGB } else { WTSAT_RGB };
    Ok((bitmap, alpha_type))
}
