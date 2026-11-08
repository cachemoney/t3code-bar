use base64::Engine;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

type FaviconKey = (Option<String>, Option<String>);
type FaviconCache = HashMap<FaviconKey, Option<String>>;

#[derive(Default)]
pub struct Favicons(Mutex<FaviconCache>);
impl Favicons {
    pub fn resolve(&self, root: Option<&str>, explicit: Option<&str>) -> Option<String> {
        let key = (root.map(str::to_owned), explicit.map(str::to_owned));
        let mut cache = self.0.lock().unwrap();
        cache
            .entry(key)
            .or_insert_with(|| {
                let root = root.filter(|s| !s.is_empty()).map(Path::new);
                let mut candidates = Vec::<PathBuf>::new();
                if let Some(path) = explicit.filter(|s| !s.is_empty()).map(Path::new) {
                    if path.is_absolute() {
                        candidates.push(path.into());
                    } else if let Some(root) = root {
                        candidates.push(root.join(path));
                    }
                }
                if let Some(root) = root {
                    for path in [
                        "favicon.svg",
                        "favicon.ico",
                        "favicon.png",
                        "public/favicon.svg",
                        "public/favicon.ico",
                        "public/favicon.png",
                        "app/favicon.ico",
                        "app/favicon.png",
                        "app/icon.svg",
                        "app/icon.png",
                        "app/icon.ico",
                        "src/favicon.ico",
                        "src/favicon.svg",
                        "src/app/favicon.ico",
                        "src/app/icon.svg",
                        "src/app/icon.png",
                        "assets/icon.svg",
                        "assets/icon.png",
                        "assets/logo.svg",
                        "assets/logo.png",
                        ".idea/icon.svg",
                        "Resources/AppIcon.icns",
                        "AppIcon.icns",
                    ] {
                        candidates.push(root.join(path));
                    }
                }
                candidates.iter().find_map(|path| data_url(path))
            })
            .clone()
    }
}
fn data_url(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    #[cfg(target_os = "macos")]
    if let Some(png) = macos_png(path) {
        return Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ));
    }
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(target_os = "macos")]
fn macos_png(path: &Path) -> Option<Vec<u8>> {
    type NativeObject = *mut objc::runtime::Object;
    let nil: NativeObject = std::ptr::null_mut();
    use objc::{class, msg_send, sel, sel_impl};
    let path = std::ffi::CString::new(path.to_str()?).ok()?;
    objc::rc::autoreleasepool(|| unsafe {
        let name: NativeObject = msg_send![class!(NSString), stringWithUTF8String: path.as_ptr()];
        let image: NativeObject = msg_send![class!(NSImage), alloc];
        let image: NativeObject = msg_send![image, initWithContentsOfFile: name];
        if image == nil {
            return None;
        }
        let _: NativeObject = msg_send![image, autorelease];
        let tiff: NativeObject = msg_send![image, TIFFRepresentation];
        if tiff == nil {
            return None;
        }
        let bitmap: NativeObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
        if bitmap == nil {
            return None;
        }
        let properties: NativeObject = msg_send![class!(NSDictionary), dictionary];
        let data: NativeObject =
            msg_send![bitmap, representationUsingType: 4usize properties: properties];
        if data == nil {
            return None;
        }
        let length: usize = msg_send![data, length];
        if length == 0 || length > 2 * 1024 * 1024 {
            return None;
        }
        let bytes: *const u8 = msg_send![data, bytes];
        if bytes.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(bytes, length).to_vec())
    })
}
