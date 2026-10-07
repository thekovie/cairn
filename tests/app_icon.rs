//! cairn.exe carries the Cairn logo, so Windows shows it on shortcuts, the
//! taskbar, and in Settings → Apps instead of the blank program icon.
#![cfg(windows)]

/// The largest picture in installer/icons/cairn.ico (stored as PNG).
fn largest_icon_png() -> Vec<u8> {
    let ico = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/installer/icons/cairn.ico"
    ))
    .unwrap();
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let entry = |i: usize| &ico[6 + 16 * i..6 + 16 * (i + 1)];
    let (size, offset) = (0..count)
        .map(|i| {
            let e = entry(i);
            let size = u32::from_le_bytes(e[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(e[12..16].try_into().unwrap()) as usize;
            (size, offset)
        })
        .max()
        .unwrap();
    ico[offset..offset + size].to_vec()
}

#[test]
fn the_program_carries_the_cairn_logo() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_cairn")).unwrap();
    let png = largest_icon_png();
    assert!(
        exe.windows(png.len()).any(|w| w == png),
        "cairn.exe doesn't contain the logo from installer/icons/cairn.ico"
    );
}
