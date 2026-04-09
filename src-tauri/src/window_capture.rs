use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use image::{
    codecs::png::PngEncoder, imageops::FilterType, ColorType, DynamicImage, ImageBuffer,
    ImageEncoder, RgbaImage,
};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
    GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
    DIB_RGB_COLORS, ROP_CODE, SRCCOPY,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, IsIconic, IsWindowVisible, PW_RENDERFULLCONTENT,
};

const MAX_IMAGE_EDGE: u32 = 1440;

#[derive(Debug, Clone)]
pub struct FocusedWindowScreenshot {
    pub mime_type: String,
    pub data_base64: String,
}

pub fn capture_focused_window_screenshot(
    app: &AppHandle,
    target_hwnd: Option<isize>,
    debug_save_copy: bool,
) -> Result<Option<FocusedWindowScreenshot>> {
    let Some(raw_hwnd) = target_hwnd else {
        return Ok(None);
    };

    if is_app_window(app, raw_hwnd) {
        tracing::debug!("window_capture: 跳过本应用窗口截图 hwnd=0x{:X}", raw_hwnd);
        return Ok(None);
    }

    let hwnd = HWND(raw_hwnd as *mut _);
    if unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() } {
        tracing::debug!(
            "window_capture: 目标窗口不可见或已最小化 hwnd=0x{:X}",
            raw_hwnd
        );
        return Ok(None);
    }

    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.context("读取窗口矩形失败")?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Ok(None);
    }

    let pixels = capture_window_pixels(hwnd, width, height)?;
    let image = encode_png_base64(pixels, width as u32, height as u32, debug_save_copy)?;
    Ok(Some(image))
}

pub fn debug_screenshot_output_path() -> PathBuf {
    std::env::temp_dir()
        .join("PushToTalkOmni")
        .join("latest-focused-window.png")
}

fn is_app_window(app: &AppHandle, hwnd: isize) -> bool {
    ["main", "overlay", "notification"].iter().any(|label| {
        app.get_webview_window(label)
            .and_then(|window| window.hwnd().ok())
            .map(|window_hwnd| window_hwnd.0 as isize == hwnd)
            .unwrap_or(false)
    })
}

fn capture_window_pixels(hwnd: HWND, width: i32, height: i32) -> Result<Vec<u8>> {
    let window_dc = unsafe { GetWindowDC(hwnd) };
    if window_dc.0.is_null() {
        return Err(anyhow!("GetWindowDC 返回空句柄"));
    }

    let mem_dc = unsafe { CreateCompatibleDC(window_dc) };
    if mem_dc.0.is_null() {
        unsafe {
            let _ = ReleaseDC(hwnd, window_dc);
        }
        return Err(anyhow!("CreateCompatibleDC 失败"));
    }

    let bitmap = unsafe { CreateCompatibleBitmap(window_dc, width, height) };
    if bitmap.0.is_null() {
        unsafe {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(hwnd, window_dc);
        }
        return Err(anyhow!("CreateCompatibleBitmap 失败"));
    }

    let old_bitmap = unsafe { SelectObject(mem_dc, bitmap) };
    let print_success =
        unsafe { PrintWindow(hwnd, mem_dc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool() };

    if !print_success {
        let rop = ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0);
        let bitblt_result = unsafe { BitBlt(mem_dc, 0, 0, width, height, window_dc, 0, 0, rop) };
        if let Err(e) = bitblt_result {
            unsafe {
                let _ = SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(hwnd, window_dc);
            }
            return Err(e).context("BitBlt fallback 失败");
        }
    }

    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    let copied_lines = unsafe {
        GetDIBits(
            mem_dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        )
    };

    unsafe {
        let _ = SelectObject(mem_dc, old_bitmap);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(hwnd, window_dc);
    }

    if copied_lines == 0 {
        return Err(anyhow!("GetDIBits 返回 0"));
    }

    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    Ok(pixels)
}

fn encode_png_base64(
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    debug_save_copy: bool,
) -> Result<FocusedWindowScreenshot> {
    let rgba =
        ImageBuffer::from_raw(width, height, pixels).context("无法从窗口像素构造 RGBA 图像")?;
    let resized = resize_if_needed(rgba);

    let mut png_bytes = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(
            resized.as_raw(),
            resized.width(),
            resized.height(),
            ColorType::Rgba8.into(),
        )
        .context("PNG 编码失败")?;

    if debug_save_copy {
        write_debug_screenshot(&png_bytes)?;
    }

    Ok(FocusedWindowScreenshot {
        mime_type: "image/png".to_string(),
        data_base64: general_purpose::STANDARD.encode(png_bytes),
    })
}

fn write_debug_screenshot(png_bytes: &[u8]) -> Result<()> {
    let output_path = debug_screenshot_output_path();
    let parent = output_path.parent().context("调试截图路径缺少父目录")?;
    fs::create_dir_all(parent).context("创建调试截图目录失败")?;
    fs::write(&output_path, png_bytes).context("写入调试截图失败")?;
    tracing::info!("window_capture: 调试截图已保存到 {}", output_path.display());
    Ok(())
}

fn resize_if_needed(image: RgbaImage) -> RgbaImage {
    if image.width().max(image.height()) <= MAX_IMAGE_EDGE {
        return image;
    }

    DynamicImage::ImageRgba8(image)
        .resize(MAX_IMAGE_EDGE, MAX_IMAGE_EDGE, FilterType::Triangle)
        .into_rgba8()
}
