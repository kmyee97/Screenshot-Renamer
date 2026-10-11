//! Local Windows OCR; call on a worker, never the UI thread.
use crate::{ocr_text_cleaner::clean_ocr_text, screenshot_metadata::ScreenshotMetadata};
use std::{fmt, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrResult {
    Text(String),
    NoText,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrError {
    UnreadableFile(String),
    UnsupportedFormat,
    EngineUnavailable,
    ProcessingFailed(String),
}
impl fmt::Display for OcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnreadableFile(message) => write!(f, "Cannot read screenshot for OCR: {message}"),
            Self::UnsupportedFormat => f.write_str("OCR supports PNG and JPEG screenshots."),
            Self::EngineUnavailable => f.write_str("Windows OCR or an installed user language is unavailable; using remaining context."),
            Self::ProcessingFailed(message) => write!(f, "OCR could not analyze this screenshot: {message}"),
        }
    }
}
impl std::error::Error for OcrError {}

pub fn enrich_metadata(metadata: &mut ScreenshotMetadata, result: Result<OcrResult, OcrError>) {
    match result {
        Ok(OcrResult::Text(raw)) => {
            let cleaned = clean_ocr_text(&raw);
            metadata.set_ocr_text(raw, cleaned);
        }
        Ok(OcrResult::NoText) => metadata.set_ocr_text(String::new(), String::new()),
        Err(error) => metadata.set_ocr_failure(error.to_string()),
    }
}

pub fn analyze(path: &Path) -> Result<OcrResult, OcrError> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| OcrError::UnreadableFile(e.to_string()))?;
    if !path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["png", "jpg", "jpeg"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    }) {
        return Err(OcrError::UnsupportedFormat);
    }
    let size = file
        .metadata()
        .map_err(|e| OcrError::UnreadableFile(e.to_string()))?
        .len();
    if size > 32 * 1024 * 1024 {
        return Err(OcrError::ProcessingFailed(
            "Image exceeds the 32 MiB OCR limit.".into(),
        ));
    }
    let mut header = [0u8; 8];
    let length = file
        .read(&mut header)
        .map_err(|e| OcrError::UnreadableFile(e.to_string()))?;
    if !(length == 8 && header == *b"\x89PNG\r\n\x1a\n"
        || length >= 3 && header[..3] == [0xff, 0xd8, 0xff])
    {
        return Err(OcrError::ProcessingFailed(
            "Invalid or corrupt image header.".into(),
        ));
    }
    drop(file);
    native_analyze(path)
}

#[cfg(not(windows))]
fn native_analyze(_: &Path) -> Result<OcrResult, OcrError> {
    Err(OcrError::EngineUnavailable)
}

#[cfg(windows)]
fn native_analyze(path: &Path) -> Result<OcrResult, OcrError> {
    use windows::{
        core::HSTRING,
        Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat},
        Media::Ocr::OcrEngine,
        Storage::{FileAccessMode, StorageFile},
        Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|_| OcrError::EngineUnavailable)?;
    let _apartment = Apartment;
    let processing = |e: windows::core::Error| OcrError::ProcessingFailed(e.to_string());
    let path = std::path::absolute(path).map_err(|e| OcrError::UnreadableFile(e.to_string()))?;
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))
        .and_then(|op| op.join())
        .map_err(|e| OcrError::UnreadableFile(e.to_string()))?;
    let stream = file
        .OpenAsync(FileAccessMode::Read)
        .and_then(|op| op.join())
        .map_err(processing)?;
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|op| op.join())
        .map_err(processing)?;
    let maximum = OcrEngine::MaxImageDimension().map_err(|_| OcrError::EngineUnavailable)?;
    if decoder.PixelWidth().map_err(processing)? > maximum
        || decoder.PixelHeight().map_err(processing)? > maximum
    {
        return Err(OcrError::ProcessingFailed(format!(
            "Image exceeds Windows OCR's {maximum} pixel dimension limit."
        )));
    }
    let bitmap = decoder
        .GetSoftwareBitmapConvertedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore)
        .and_then(|op| op.join())
        .map_err(processing)?;
    let engine =
        OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| OcrError::EngineUnavailable)?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .and_then(|op| op.join())
        .map_err(processing)?;
    let text = result.Text().map_err(processing)?.to_string();
    Ok(if text.trim().is_empty() {
        OcrResult::NoText
    } else {
        OcrResult::Text(text)
    })
}
