//! Conversions between `tuinix` and `termnix` geometry, and the split of the
//! screen into a terminal grid (top) and a software keyboard (bottom).

use std::num::NonZeroU16;

/// The number of rows the keyboard occupies for a screen of `terminal` size.
///
/// The keyboard's vertical extent comes from the layout: it is the largest
/// bottom edge (`position.row + size.rows`) over every key and the preview.
/// The keyboard sits at the bottom of the screen, so its rows are the bottom
/// `keyboard_rows(..)` rows; the grid gets everything above them.
pub fn keyboard_rows(layout_rows: usize) -> usize {
    layout_rows
}

/// The number of rows left for the terminal grid when the keyboard takes the
/// bottom `keyboard_rows` rows of a `terminal_rows`-row screen.
pub fn grid_rows(terminal_rows: usize, keyboard_rows: usize) -> usize {
    terminal_rows.saturating_sub(keyboard_rows)
}

/// The column at which a `layout_cols`-wide keyboard is drawn so it is centred
/// in a `terminal_cols`-wide terminal.
///
/// The keyboard's own width comes from the layout (its absolute column
/// coordinates), while the centring uses the terminal's measured width, so a
/// layout authored for one width still lands centred on another.
pub fn keyboard_offset_col(terminal_cols: usize, layout_cols: usize) -> usize {
    terminal_cols.saturating_sub(layout_cols) / 2
}

/// Converts a `tuinix` size to a `termnix` size.
///
/// Returns `None` when either dimension is zero, because `termnix` sizes are
/// [`NonZeroU16`] and an unaddressable zero-sized grid cannot be represented.
/// The dimensions are also clamped to `u16`.
pub fn to_termnix_size(size: tuinix::Size) -> Option<termnix::Size> {
    let rows = u16::try_from(size.rows).ok().and_then(NonZeroU16::new)?;
    let cols = u16::try_from(size.cols).ok().and_then(NonZeroU16::new)?;
    Some(termnix::Size { rows, cols })
}

/// Converts a `termnix` size to a `tuinix` size.
pub fn from_termnix_size(size: termnix::Size) -> tuinix::Size {
    tuinix::Size {
        rows: size.rows.get() as usize,
        cols: size.cols.get() as usize,
    }
}
