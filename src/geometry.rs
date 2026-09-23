//! Conversions between `tuinix` and `termnix` geometry.

use std::num::NonZeroU16;

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
