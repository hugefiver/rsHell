use crate::SmokePngEvidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeByteOrder {
    Little,
    Big,
}

impl NativeByteOrder {
    pub const fn current() -> Self {
        #[cfg(target_endian = "little")]
        {
            Self::Little
        }
        #[cfg(target_endian = "big")]
        {
            Self::Big
        }
    }
}

pub fn argb32_native_to_rgba(
    bytes: &[u8],
    order: NativeByteOrder,
) -> Result<Vec<u8>, &'static str> {
    if !bytes.len().is_multiple_of(4) {
        return Err("argb32_length_invalid");
    }
    let mut rgba = Vec::with_capacity(bytes.len());
    for pixel in bytes.chunks_exact(4) {
        let (a, red, green, blue) = match order {
            NativeByteOrder::Little => (pixel[3], pixel[2], pixel[1], pixel[0]),
            NativeByteOrder::Big => (pixel[0], pixel[1], pixel[2], pixel[3]),
        };
        let unpremultiply = |channel: u8| match a {
            0 => 0,
            255 => channel,
            _ => (((u32::from(channel) * 255 + u32::from(a) / 2) / u32::from(a)).min(255)) as u8,
        };
        rgba.extend_from_slice(&[
            unpremultiply(red),
            unpremultiply(green),
            unpremultiply(blue),
            a,
        ]);
    }
    Ok(rgba)
}

pub fn analyze_rgba(
    rgba: &[u8],
    width: i32,
    height: i32,
) -> Result<SmokePngEvidence, &'static str> {
    let (width, height) = dimensions(width, height, rgba.len())?;
    let (luminance_buckets, span) = luminance_distribution(rgba);
    if luminance_buckets < 8 || span < 32 {
        return Err("visual_luminance_range_invalid");
    }
    // Native decorations occupy the first 5%; application chrome begins below them.
    let regions = [
        region(width, height, 2, 98, 5, 10),
        region(width, height, 1, 16, 10, 75),
        region(width, height, 20, 90, 10, 15),
        region(width, height, 20, 90, 15, 20),
    ];
    let dark_regions_passed = regions
        .into_iter()
        .filter(|bounds| dark_ratio(rgba, width, *bounds) >= 0.65)
        .count();
    if dark_regions_passed != regions.len() {
        return Err("visual_dark_regions_invalid");
    }
    let accent_region = region(width, height, 20, 90, 5, 15);
    let thickness = accent_thickness(rgba, width, accent_region);
    if !(2..=4).contains(&thickness) {
        return Err("visual_focus_thickness_invalid");
    }
    Ok(SmokePngEvidence {
        width: width as i32,
        height: height as i32,
        non_empty: true,
        luminance_buckets,
        dark_regions_required: regions.len(),
        dark_regions_passed,
        focus_or_selection_thickness_px: thickness,
    })
}

fn dimensions(width: i32, height: i32, length: usize) -> Result<(usize, usize), &'static str> {
    let width = usize::try_from(width).map_err(|_| "visual_dimensions_invalid")?;
    let height = usize::try_from(height).map_err(|_| "visual_dimensions_invalid")?;
    let expected = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("visual_dimensions_invalid")?;
    if width == 0 || height == 0 || length != expected {
        return Err("visual_dimensions_invalid");
    }
    Ok((width, height))
}

fn luminance_distribution(rgba: &[u8]) -> (usize, usize) {
    let mut occupied = [false; 256];
    for pixel in rgba.chunks_exact(4).filter(|pixel| pixel[3] >= 230) {
        let luminance = (2_126 * usize::from(pixel[0])
            + 7_152 * usize::from(pixel[1])
            + 722 * usize::from(pixel[2]))
            / 10_000;
        occupied[luminance] = true;
    }
    let first = occupied.iter().position(|value| *value).unwrap_or(0);
    let last = occupied.iter().rposition(|value| *value).unwrap_or(0);
    (
        occupied.into_iter().filter(|value| *value).count(),
        last - first,
    )
}

#[derive(Clone, Copy)]
struct Region {
    x_start: usize,
    x_end: usize,
    y_start: usize,
    y_end: usize,
}

fn region(
    width: usize,
    height: usize,
    x_start: usize,
    x_end: usize,
    y_start: usize,
    y_end: usize,
) -> Region {
    Region {
        x_start: width * x_start / 100,
        x_end: width * x_end / 100,
        y_start: height * y_start / 100,
        y_end: height * y_end / 100,
    }
}

fn dark_ratio(rgba: &[u8], width: usize, bounds: Region) -> f64 {
    let mut dark = 0usize;
    let mut total = 0usize;
    for pixel in pixels(rgba, width, bounds) {
        let min = pixel[0].min(pixel[1]).min(pixel[2]);
        let max = pixel[0].max(pixel[1]).max(pixel[2]);
        dark += usize::from(pixel[3] >= 230 && max <= 88 && max - min <= 28);
        total += 1;
    }
    dark as f64 / total.max(1) as f64
}

fn accent_thickness(rgba: &[u8], width: usize, bounds: Region) -> usize {
    let row_flags = (bounds.y_start..bounds.y_end).map(|y| {
        (bounds.x_start..bounds.x_end)
            .filter(|x| cyan(pixel(rgba, width, *x, y)))
            .count()
            >= 16
    });
    let column_flags = (bounds.x_start..bounds.x_end).map(|x| {
        (bounds.y_start..bounds.y_end)
            .filter(|y| cyan(pixel(rgba, width, x, *y)))
            .count()
            >= 16
    });
    max_run(row_flags).max(max_run(column_flags))
}

fn max_run(flags: impl Iterator<Item = bool>) -> usize {
    flags
        .fold((0usize, 0usize), |(best, current), flag| {
            let current = if flag { current + 1 } else { 0 };
            (best.max(current), current)
        })
        .0
}

fn cyan(pixel: &[u8]) -> bool {
    pixel[0] <= 150 && pixel[1] >= 140 && pixel[2] >= 190 && pixel[2] >= pixel[1] && pixel[3] >= 230
}

fn pixels(rgba: &[u8], width: usize, bounds: Region) -> impl Iterator<Item = &[u8]> {
    (bounds.y_start..bounds.y_end)
        .flat_map(move |y| (bounds.x_start..bounds.x_end).map(move |x| pixel(rgba, width, x, y)))
}

fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    let offset = (y * width + x) * 4;
    &rgba[offset..offset + 4]
}

#[cfg(test)]
#[path = "visual_png_tests.rs"]
mod tests;
