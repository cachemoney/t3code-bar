#![allow(deprecated, unexpected_cfgs)]

use bar_core::threads::ThreadStatus;
use block::ConcreteBlock;
use cocoa::{
    base::{id, nil, BOOL, NO, YES},
    foundation::{NSAutoreleasePool, NSData, NSPoint, NSRect, NSSize, NSString},
};
use objc::{class, msg_send, rc::StrongPtr, sel, sel_impl};

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    static NSFontAttributeName: id;
    static NSForegroundColorAttributeName: id;
    static NSFontDescriptorSystemDesignRounded: id;
    static NSFontWeightRegular: f64;
    static NSFontWeightMedium: f64;
}

// T3 Code identity artwork retained in the standalone desktop crate.
const LOGO: &[u8] = include_bytes!("../resources/T3Logo.png");
const HEIGHT: f64 = 18.0;

#[derive(Debug, PartialEq)]
enum Label {
    None,
    Count(String),
    Capsule(String),
}

struct Style {
    dot: Option<[u8; 3]>,
    label: Label,
}

impl Style {
    fn new(status: ThreadStatus, count: usize) -> Self {
        use ThreadStatus::*;
        let (name, color, attention) = match status {
            Working => ("Working", [96, 165, 250], false),
            Goal => ("Goal", [96, 165, 250], false),
            Approval => ("Approval", [251, 191, 36], true),
            Input => ("Input", [165, 180, 252], true),
            Request => ("Request", [240, 171, 252], true),
            Woke => ("Woke", [251, 191, 36], true),
            Failed => ("Failed", [248, 113, 113], true),
            Limited => ("Limited", [251, 191, 36], true),
            Waiting => ("Waiting", [163, 163, 163], false),
            Done => ("Done", [52, 211, 153], false),
            Settled => ("Settled", [139, 139, 148], false),
        };
        let label = if attention {
            Label::Capsule(if count > 1 {
                format!("{name} {count}")
            } else {
                name.into()
            })
        } else if count > 1 {
            Label::Count(count.to_string())
        } else {
            Label::None
        };
        Self {
            dot: (status != Settled).then_some(color),
            label,
        }
    }
}

pub fn update(
    tray: &tauri::tray::TrayIcon,
    status: ThreadStatus,
    count: usize,
) -> Result<(), String> {
    tray.with_inner_tray_icon(move |tray| unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let result = (|| {
            let item = tray.ns_status_item().ok_or("Status item is absent")?;
            let item = (&*item) as *const _ as id;
            let button: id = msg_send![item, button];
            if button == nil {
                return Err("Status button is absent".into());
            }
            let image = image(Style::new(status, count))?;
            let empty = NSString::alloc(nil).init_str("");
            if empty == nil {
                return Err("Native tray title cannot be created".into());
            }
            let empty = StrongPtr::new(empty);
            let _: () = msg_send![button, setTitle: *empty];
            let _: () = msg_send![button, setImage: *image];
            let _: () = msg_send![button, setImagePosition: 1u64];
            Ok(())
        })();
        pool.drain();
        result
    })
    .map_err(|error| error.to_string())?
}

unsafe fn color(rgb: [u8; 3]) -> id {
    msg_send![class!(NSColor), colorWithSRGBRed: rgb[0] as f64 / 255.0
        green: rgb[1] as f64 / 255.0 blue: rgb[2] as f64 / 255.0 alpha: 1.0f64]
}

unsafe fn rounded_font(capsule: bool) -> Result<id, String> {
    let font: id = msg_send![class!(NSFont), systemFontOfSize: 12.0f64
        weight: if capsule { NSFontWeightRegular } else { NSFontWeightMedium }];
    if font == nil {
        return Err("Native tray font cannot be created".into());
    }
    let descriptor: id = msg_send![font, fontDescriptor];
    if descriptor == nil {
        return Err("Native tray font descriptor cannot be created".into());
    }
    let rounded: id =
        msg_send![descriptor, fontDescriptorWithDesign: NSFontDescriptorSystemDesignRounded];
    if rounded == nil {
        return Err("Native rounded tray font cannot be created".into());
    }
    let result: id = msg_send![class!(NSFont), fontWithDescriptor: rounded size: 12.0f64];
    if result == nil {
        return Err("Native rounded tray font cannot be created".into());
    }
    Ok(result)
}

unsafe fn image(style: Style) -> Result<StrongPtr, String> {
    let data = NSData::dataWithBytes_length_(nil, LOGO.as_ptr().cast(), LOGO.len() as u64);
    if data == nil {
        return Err("Embedded T3Logo.png data cannot be created".into());
    }
    let logo: id = msg_send![class!(NSImage), alloc];
    if logo == nil {
        return Err("Native tray logo cannot be allocated".into());
    }
    let logo: id = msg_send![logo, initWithData: data];
    if logo == nil {
        return Err("Embedded T3Logo.png cannot be decoded".into());
    }
    let logo = StrongPtr::new(logo);
    let capsule = matches!(style.label, Label::Capsule(_));
    let (text, attributes, text_size) = match &style.label {
        Label::None => (None, None, NSSize::new(0.0, 0.0)),
        Label::Count(text) | Label::Capsule(text) => {
            let text = NSString::alloc(nil).init_str(text);
            if text == nil {
                return Err("Native tray label cannot be created".into());
            }
            let text = StrongPtr::new(text);
            let foreground: id = if capsule {
                msg_send![class!(NSColor), colorWithWhite: 0.0f64 alpha: 0.85f64]
            } else {
                msg_send![class!(NSColor), labelColor]
            };
            if foreground == nil {
                return Err("Native tray label color cannot be created".into());
            }
            let font = rounded_font(capsule)?;
            let objects = [font, foreground];
            let keys = [NSFontAttributeName, NSForegroundColorAttributeName];
            let attributes: id = msg_send![class!(NSDictionary), dictionaryWithObjects: objects.as_ptr()
                forKeys: keys.as_ptr() count: 2u64];
            if attributes == nil {
                return Err("Native tray label attributes cannot be created".into());
            }
            let attributes = StrongPtr::retain(attributes);
            let size: NSSize = msg_send![*text, sizeWithAttributes: *attributes];
            (Some(text), Some(attributes), size)
        }
    };
    let label_width = text_size.width.ceil() + if capsule { 12.0 } else { 0.0 };
    let width = (HEIGHT
        + if text.is_some() {
            3.0 + label_width
        } else {
            0.0
        })
    .ceil();
    // NSImage's drawing handler preserves point geometry at every backing scale and appearance.
    let drawing = ConcreteBlock::new(move |_rect: NSRect| -> BOOL {
        let logo_rect = rect(0.0, 0.0, HEIGHT, HEIGHT);
        let _: () = msg_send![*logo, drawInRect: logo_rect fromRect: rect(0.0, 0.0, 0.0, 0.0)
            operation: 2u64 fraction: 1.0f64];
        if let Some(rgb) = style.dot {
            let black: id = msg_send![class!(NSColor), blackColor];
            let _: () = msg_send![black, setFill];
            let outline: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect(11.25, 0.25, 6.5, 6.5)];
            let _: () = msg_send![outline, fill];
            let _: () = msg_send![color(rgb), setFill];
            let dot: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect(12.0, 1.0, 5.0, 5.0)];
            let _: () = msg_send![dot, fill];
        }
        if let (Some(text), Some(attributes)) = (&text, &attributes) {
            let label_height = text_size.height + if capsule { 2.0 } else { 0.0 };
            let y = (HEIGHT - label_height) / 2.0;
            if capsule {
                if let Some(rgb) = style.dot {
                    let _: () = msg_send![color(rgb), setFill];
                }
                let path: id = msg_send![class!(NSBezierPath), bezierPathWithRoundedRect:
                    rect(21.0, y, label_width, label_height)
                    xRadius: label_height / 2.0 yRadius: label_height / 2.0];
                let _: () = msg_send![path, fill];
            }
            let point = NSPoint::new(21.0 + if capsule { 6.0 } else { 0.0 }, (HEIGHT - text_size.height) / 2.0);
            let _: () = msg_send![**text, drawAtPoint: point withAttributes: **attributes];
        }
        YES
    })
    .copy();
    let image: id = msg_send![class!(NSImage), imageWithSize: NSSize::new(width, HEIGHT)
        flipped: NO drawingHandler: &*drawing];
    if image == nil {
        return Err("Native tray image cannot be created".into());
    }
    let _: () = msg_send![image, setTemplate: NO];
    let _: () = msg_send![image, setCacheMode: 3u64];
    Ok(StrongPtr::retain(image))
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn raster(image: id, scale: f64) -> StrongPtr {
        let size: NSSize = msg_send![image, size];
        let space = StrongPtr::new(NSString::alloc(nil).init_str("NSCalibratedRGBColorSpace"));
        let bitmap: id = msg_send![class!(NSBitmapImageRep), alloc];
        let bitmap: id = msg_send![bitmap, initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>()
            pixelsWide: (size.width * scale).ceil() as i64 pixelsHigh: (size.height * scale) as i64
            bitsPerSample: 8i64 samplesPerPixel: 4i64 hasAlpha: YES isPlanar: NO
            colorSpaceName: *space bytesPerRow: 0i64 bitsPerPixel: 0i64];
        assert_ne!(bitmap, nil);
        let bitmap = StrongPtr::new(bitmap);
        let context: id =
            msg_send![class!(NSGraphicsContext), graphicsContextWithBitmapImageRep: *bitmap];
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: context];
        let transform: id = msg_send![class!(NSAffineTransform), transform];
        let _: () = msg_send![transform, scaleBy: scale];
        let _: () = msg_send![transform, concat];
        let _: () = msg_send![image, drawInRect: rect(0.0, 0.0, size.width, size.height)
            fromRect: rect(0.0, 0.0, 0.0, 0.0) operation: 2u64 fraction: 1.0f64];
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        let srgb: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        let converted: id =
            msg_send![*bitmap, bitmapImageRepByConvertingToColorSpace: srgb renderingIntent: 0u64];
        StrongPtr::retain(converted)
    }

    unsafe fn pixel(bitmap: id, x: usize, y: usize) -> [u8; 4] {
        let data: *const u8 = msg_send![bitmap, bitmapData];
        let stride: usize = msg_send![bitmap, bytesPerRow];
        std::slice::from_raw_parts(data.add(y * stride + x * 4), 4)
            .try_into()
            .unwrap()
    }

    #[test]
    fn primary_count_follows_the_drawing_appearance() {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let image = image(Style::new(ThreadStatus::Working, 12)).unwrap();
            let mut brightness = Vec::new();
            for name in ["NSAppearanceNameAqua", "NSAppearanceNameDarkAqua"] {
                let name = StrongPtr::new(NSString::alloc(nil).init_str(name));
                let appearance: id = msg_send![class!(NSAppearance), appearanceNamed: *name];
                assert_ne!(appearance, nil);
                let sum = std::cell::Cell::new(0u64);
                let drawing = ConcreteBlock::new(|| {
                    let bitmap = raster(*image, 2.0);
                    let width: usize = msg_send![*bitmap, pixelsWide];
                    for y in 0..36 {
                        for x in 42..width {
                            let rgba = pixel(*bitmap, x, y);
                            sum.set(sum.get() + rgba[..3].iter().map(|c| *c as u64).sum::<u64>());
                        }
                    }
                });
                let _: () = msg_send![appearance, performAsCurrentDrawingAppearance: &drawing];
                brightness.push(sum.get());
            }
            assert!(brightness[1] > brightness[0], "{brightness:?}");
            drop(image);
            pool.drain();
        }
    }

    #[test]
    fn native_images_draw_logo_dot_capsule_and_count_at_both_scales() {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let cases = [
                (ThreadStatus::Settled, 1),
                (ThreadStatus::Working, 1),
                (ThreadStatus::Working, 12),
                (ThreadStatus::Goal, 2),
                (ThreadStatus::Approval, 1),
                (ThreadStatus::Input, 2),
                (ThreadStatus::Request, 12),
                (ThreadStatus::Woke, 2),
                (ThreadStatus::Failed, 1),
                (ThreadStatus::Limited, 2),
                (ThreadStatus::Waiting, 2),
                (ThreadStatus::Done, 2),
            ];
            for scale in [1.0, 2.0] {
                let settled = image(Style::new(ThreadStatus::Settled, 1)).unwrap();
                let reference = raster(*settled, scale);
                for (status, count) in cases {
                    let style = Style::new(status, count);
                    let dot = style.dot;
                    let capsule = matches!(style.label, Label::Capsule(_));
                    let has_label = !matches!(style.label, Label::None);
                    let inner_pool = NSAutoreleasePool::new(nil);
                    let image = image(style).unwrap();
                    inner_pool.drain();
                    let size: NSSize = msg_send![*image, size];
                    assert_eq!(size.height, HEIGHT);
                    assert_eq!(size.width > HEIGHT, has_label);
                    let template: BOOL = msg_send![*image, isTemplate];
                    assert_eq!(template, NO);
                    let bitmap = raster(*image, scale);
                    for y in 0..(HEIGHT * scale) as usize {
                        for x in 0..(11.0 * scale) as usize {
                            assert_eq!(pixel(*bitmap, x, y), pixel(*reference, x, y));
                        }
                    }
                    if let Some(rgb) = dot {
                        let center =
                            pixel(*bitmap, (14.5 * scale) as usize, (14.5 * scale) as usize);
                        for (actual, expected) in center[..3].iter().zip(rgb) {
                            assert!(
                                (*actual as i16 - expected as i16).abs() <= 2,
                                "{status:?} dot {center:?}"
                            );
                        }
                        assert_eq!(center[3], 255);
                    }
                    if capsule {
                        assert_eq!(
                            pixel(*bitmap, (23.0 * scale) as usize, (9.0 * scale) as usize)[3],
                            255
                        );
                    }
                    if let Some(directory) = std::env::var_os("T3_BAR_TRAY_RENDER_DIR") {
                        let directory = std::path::PathBuf::from(directory);
                        std::fs::create_dir_all(&directory).unwrap();
                        let properties: id = msg_send![class!(NSDictionary), dictionary];
                        let png: id = msg_send![*bitmap, representationUsingType: 4u64 properties: properties];
                        let data: *const u8 = msg_send![png, bytes];
                        let length: usize = msg_send![png, length];
                        std::fs::write(
                            directory.join(format!("{status:?}-{count}@{scale}x.png")),
                            std::slice::from_raw_parts(data, length),
                        )
                        .unwrap();
                    }
                }
            }
            pool.drain();
        }
    }

    #[test]
    fn labels_follow_the_swift_status_contract() {
        for (status, name) in [
            (ThreadStatus::Approval, "Approval"),
            (ThreadStatus::Input, "Input"),
            (ThreadStatus::Request, "Request"),
            (ThreadStatus::Woke, "Woke"),
            (ThreadStatus::Failed, "Failed"),
            (ThreadStatus::Limited, "Limited"),
        ] {
            for count in [0, 1] {
                assert_eq!(Style::new(status, count).label, Label::Capsule(name.into()));
            }
            assert_eq!(
                Style::new(status, 2).label,
                Label::Capsule(format!("{name} 2"))
            );
        }
        for status in [
            ThreadStatus::Working,
            ThreadStatus::Goal,
            ThreadStatus::Waiting,
            ThreadStatus::Done,
            ThreadStatus::Settled,
        ] {
            assert_eq!(Style::new(status, 1).label, Label::None);
            assert_eq!(Style::new(status, 12).label, Label::Count("12".into()));
        }
        assert_eq!(Style::new(ThreadStatus::Settled, 1).dot, None);
    }
}
