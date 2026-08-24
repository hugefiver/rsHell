use super::{
    NativeByteOrder, PixelRegion, analyze_rgba, analyze_rgba_in_region, argb32_native_to_rgba,
};

const WIDTH: usize = 1_360;
const HEIGHT: usize = 860;

#[test]
fn argb32_native_conversion_canonicalizes_transparent_semitransparent_red_and_cyan() {
    let expected = [0, 0, 0, 0, 255, 0, 0, 128, 255, 0, 0, 255, 0, 255, 255, 255];
    let little = [0, 0, 0, 0, 0, 0, 128, 128, 0, 0, 255, 255, 255, 255, 0, 255];
    let big = [0, 0, 0, 0, 128, 128, 0, 0, 255, 255, 0, 0, 255, 0, 255, 255];
    assert_eq!(
        argb32_native_to_rgba(&little, NativeByteOrder::Little).unwrap(),
        expected
    );
    assert_eq!(
        argb32_native_to_rgba(&big, NativeByteOrder::Big).unwrap(),
        expected
    );
    assert!(argb32_native_to_rgba(&[0, 1, 2], NativeByteOrder::Little).is_err());
}

#[test]
fn canonical_rgba_ranges_require_dark_regions_and_two_pixel_treatment() {
    let evidence = analyze_rgba(&fixture_rgba(), WIDTH as i32, HEIGHT as i32).unwrap();
    assert!(evidence.non_empty);
    assert_eq!(evidence.dark_regions_passed, 4);
    assert_eq!(evidence.dark_regions_required, 4);
    assert!((2..=4).contains(&evidence.focus_or_selection_thickness_px));
    assert!(analyze_rgba(&one_color(), WIDTH as i32, HEIGHT as i32).is_err());
    assert!(analyze_rgba(&failed_dark_region(), WIDTH as i32, HEIGHT as i32).is_err());
    assert!(analyze_rgba(&accent_thickness(1), WIDTH as i32, HEIGHT as i32).is_err());
    assert!(analyze_rgba(&accent_thickness(5), WIDTH as i32, HEIGHT as i32).is_err());
}

#[test]
fn accent_analysis_uses_the_real_active_tab_bounds_not_window_percentages() {
    let mut rgba = fixture_rgba();
    fill_rect(&mut rgba, 300, 1_000, 70, 72, [34, 34, 34, 255]);
    fill_rect(&mut rgba, 40, 240, 220, 222, [96, 180, 220, 255]);
    let region = PixelRegion::new(40, 240, 190, 223);

    assert!(analyze_rgba(&rgba, WIDTH as i32, HEIGHT as i32).is_err());
    let evidence = analyze_rgba_in_region(&rgba, WIDTH as i32, HEIGHT as i32, region).unwrap();
    assert_eq!(evidence.focus_or_selection_thickness_px, 2);
}

fn fixture_rgba() -> Vec<u8> {
    let mut rgba = vec![0; WIDTH * HEIGHT * 4];
    for y in 0..HEIGHT {
        let shade = 24 + ((y % 8) * 5) as u8;
        fill_rect(&mut rgba, 0, WIDTH, y, y + 1, [shade, shade, shade, 255]);
    }
    fill_rect(&mut rgba, 300, 1_000, 70, 72, [96, 180, 220, 255]);
    rgba
}

fn one_color() -> Vec<u8> {
    [32, 32, 32, 255].repeat(WIDTH * HEIGHT)
}

fn failed_dark_region() -> Vec<u8> {
    let mut rgba = fixture_rgba();
    fill_rect(&mut rgba, 27, 1_333, 43, 86, [240, 240, 240, 255]);
    rgba
}

fn accent_thickness(thickness: usize) -> Vec<u8> {
    let mut rgba = fixture_rgba();
    fill_rect(&mut rgba, 300, 1_000, 68, 76, [34, 34, 34, 255]);
    fill_rect(
        &mut rgba,
        300,
        1_000,
        70,
        70 + thickness,
        [96, 180, 220, 255],
    );
    rgba
}

fn fill_rect(
    rgba: &mut [u8],
    x_start: usize,
    x_end: usize,
    y_start: usize,
    y_end: usize,
    color: [u8; 4],
) {
    for y in y_start..y_end {
        for x in x_start..x_end {
            let offset = (y * WIDTH + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&color);
        }
    }
}
