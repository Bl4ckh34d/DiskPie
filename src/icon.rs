/// The sidebar's purple disk and mint upper-right slice, rendered with transparency.
/// Shared by the native window and the executable's generated icon resource.
pub fn rgba(size: u32) -> Vec<u8> {
    assert!((1..=256).contains(&size));
    const SAMPLES: u32 = 4;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0_u32; 3];
            let mut covered = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    // Match the original 31-point sidebar mark (radius 15).
                    let px =
                        (x as f64 + (sx as f64 + 0.5) / SAMPLES as f64) * 31.0 / size as f64 - 15.5;
                    let py =
                        (y as f64 + (sy as f64 + 0.5) / SAMPLES as f64) * 31.0 / size as f64 - 15.5;
                    if px * px + py * py > 225.0 {
                        continue;
                    }
                    let mint = px >= 0.0
                        && py <= 0.0
                        && py >= -15.0 + px * (4.4 / 10.6)
                        && py >= (px - 15.0) * (10.6 / 4.4);
                    let color = if mint {
                        [118, 221, 189]
                    } else {
                        [181, 165, 255]
                    };
                    for channel in 0..3 {
                        sum[channel] += color[channel];
                    }
                    covered += 1;
                }
            }
            for value in sum {
                pixels.push(value.checked_div(covered).unwrap_or(0) as u8);
            }
            pixels.push((covered * 255 / (SAMPLES * SAMPLES)) as u8);
        }
    }
    pixels
}
