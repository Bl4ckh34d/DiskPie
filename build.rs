#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("diskpie.ico");
    std::fs::write(&path, windows_icon()).expect("write generated DiskPie icon");
    winresource::WindowsResource::new()
        .set_icon(path.to_str().expect("icon build path"))
        .set("ProductName", "DiskPie")
        .set("FileDescription", "DiskPie storage explorer")
        .set("OriginalFilename", "DiskPie.exe")
        .compile()
        .expect("embed DiskPie Windows icon");
}

fn windows_icon() -> Vec<u8> {
    let sizes = [16_u32, 24, 32, 48, 64, 128, 256];
    let mut directory = vec![0, 0, 1, 0, sizes.len() as u8, 0];
    let mut images = Vec::new();
    let mut offset = 6 + 16 * sizes.len() as u32;
    for size in sizes {
        let pixels = icon::rgba(size);
        let mask_stride = size.div_ceil(32) * 4;
        let image_size = size * size * 4 + mask_stride * size;
        let mut bitmap = Vec::new();
        for value in [40, size, size * 2] {
            bitmap.extend(value.to_le_bytes());
        }
        bitmap.extend(1_u16.to_le_bytes());
        bitmap.extend(32_u16.to_le_bytes());
        for value in [0, image_size, 0, 0, 0, 0] {
            bitmap.extend(value.to_le_bytes());
        }
        for y in (0..size).rev() {
            for x in 0..size {
                let i = ((y * size + x) * 4) as usize;
                bitmap.extend([pixels[i + 2], pixels[i + 1], pixels[i], pixels[i + 3]]);
            }
        }
        for y in (0..size).rev() {
            let mut mask = vec![0_u8; mask_stride as usize];
            for x in 0..size {
                if pixels[((y * size + x) * 4 + 3) as usize] == 0 {
                    mask[x as usize / 8] |= 0x80 >> (x % 8);
                }
            }
            bitmap.extend(mask);
        }
        directory.extend([size as u8, size as u8, 0, 0]);
        directory.extend(1_u16.to_le_bytes());
        directory.extend(32_u16.to_le_bytes());
        directory.extend((bitmap.len() as u32).to_le_bytes());
        directory.extend(offset.to_le_bytes());
        offset += bitmap.len() as u32;
        images.extend(bitmap);
    }
    directory.extend(images);
    directory
}
