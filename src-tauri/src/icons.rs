//! Reads the icon of a program or file from Windows and returns it as a
//! `data:image/png;base64,...` URL the frontend can show in an <img>.
//!
//! Results are cached per path for the lifetime of the app, so switching
//! projects doesn't re-read the same executables.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Pixel size to extract. Shown at 16 CSS px, so this stays sharp at 200% scaling.
const ICON_SIZE: u32 = 32;

fn cache() -> &'static Mutex<HashMap<PathBuf, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Icon of `path` as a PNG data URL, or None if Windows can't provide one.
pub fn icon_data_url(path: &Path) -> Option<String> {
    if let Some(hit) = cache().lock().ok()?.get(path) {
        return hit.clone();
    }
    let url = extract_rgba(path).and_then(|(w, h, rgba)| to_data_url(w, h, &rgba));
    if let Ok(mut c) = cache().lock() {
        c.insert(path.to_path_buf(), url.clone());
    }
    url
}

fn to_data_url(w: u32, h: u32, rgba: &[u8]) -> Option<String> {
    use base64::Engine;
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

#[cfg(not(windows))]
fn extract_rgba(_path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    None
}

#[cfg(windows)]
fn extract_rgba(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::{
        SHDefExtractIconW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        // The shell APIs want COM on this thread. If it's already initialized in
        // another mode this fails harmlessly and we just don't uninitialize.
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let com_ok = hr >= 0;

        // Executables and .ico files: ask for the exact size we want.
        let mut icon: HICON = std::ptr::null_mut();
        if SHDefExtractIconW(
            wide.as_ptr(),
            0,
            0,
            &mut icon,
            std::ptr::null_mut(),
            ICON_SIZE,
        ) != 0
        {
            icon = std::ptr::null_mut();
        }
        // Anything else (e.g. a .uproject): the icon Explorer shows for it.
        if icon.is_null() {
            let mut info: SHFILEINFOW = std::mem::zeroed();
            if SHGetFileInfoW(
                wide.as_ptr(),
                0,
                &mut info,
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            ) != 0
            {
                icon = info.hIcon;
            }
        }

        let result = if icon.is_null() {
            None
        } else {
            let r = hicon_to_rgba(icon);
            DestroyIcon(icon);
            r
        };
        if com_ok {
            CoUninitialize();
        }
        result
    }
}

/// Converts an HICON to straight RGBA pixels (top row first).
#[cfg(windows)]
unsafe fn hicon_to_rgba(
    icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut ii: ICONINFO = std::mem::zeroed();
    if GetIconInfo(icon, &mut ii) == 0 {
        return None;
    }
    let cleanup = |ii: &ICONINFO| {
        if !ii.hbmColor.is_null() {
            DeleteObject(ii.hbmColor);
        }
        if !ii.hbmMask.is_null() {
            DeleteObject(ii.hbmMask);
        }
    };
    // Monochrome icons have no color bitmap; not worth supporting.
    if ii.hbmColor.is_null() {
        cleanup(&ii);
        return None;
    }

    let mut bm: BITMAP = std::mem::zeroed();
    if GetObjectW(
        ii.hbmColor,
        std::mem::size_of::<BITMAP>() as i32,
        &mut bm as *mut _ as *mut _,
    ) == 0
    {
        cleanup(&ii);
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if w <= 0 || h <= 0 || w > 1024 || h > 1024 {
        cleanup(&ii);
        return None;
    }

    let dc: HDC = CreateCompatibleDC(std::ptr::null_mut());
    // Reads a bitmap as 32-bit BGRA, top-down.
    let read = |hbm: HBITMAP| -> Option<Vec<u8>> {
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut px = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(
            dc,
            hbm,
            0,
            h as u32,
            px.as_mut_ptr() as *mut _,
            &mut bi,
            DIB_RGB_COLORS,
        );
        if lines == h {
            Some(px)
        } else {
            None
        }
    };

    let color = read(ii.hbmColor);
    let mut out = None;
    if let Some(mut px) = color {
        let has_alpha = px.chunks_exact(4).any(|p| p[3] != 0);
        if !has_alpha {
            // Old-style icon: transparency comes from the mask (white = transparent).
            let mask = if ii.hbmMask.is_null() {
                None
            } else {
                read(ii.hbmMask)
            };
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                let transparent = mask.as_ref().map(|m| m[i * 4] != 0).unwrap_or(false);
                p[3] = if transparent { 0 } else { 255 };
            }
        }
        // BGRA -> RGBA
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        out = Some((w as u32, h as u32, px));
    }

    DeleteDC(dc);
    cleanup(&ii);
    out
}
