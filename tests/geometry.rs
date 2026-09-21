//! Properties of the terminal/keyboard geometry helpers.

#[test]
fn grid_rows_leaves_the_keyboard_at_the_bottom() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let terminal_rows = noprop::sample_usize_in(ctx, 0..=24);
        let keyboard_rows = noprop::sample_usize_in(ctx, 0..=terminal_rows);

        let grid = tuke::geometry::grid_rows(terminal_rows, keyboard_rows);

        // The grid and the keyboard together fill the screen exactly: the
        // keyboard is bottom-aligned, so nothing is lost and nothing overlaps.
        assert_eq!(
            grid + keyboard_rows,
            terminal_rows,
            "grid {grid} + keyboard {keyboard_rows} != terminal {terminal_rows}"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn grid_rows_saturates_when_the_keyboard_is_too_tall() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let terminal_rows = noprop::sample_usize_in(ctx, 0..=24);
        let extra = noprop::sample_usize_in(ctx, 0..=24);
        let keyboard_rows = terminal_rows.saturating_add(extra);

        // A keyboard taller than the screen leaves no grid rows, never a
        // negative count.
        assert_eq!(tuke::geometry::grid_rows(terminal_rows, keyboard_rows), 0);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn grid_rows_is_monotonic_in_the_keyboard_height() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let terminal_rows = noprop::sample_usize_in(ctx, 0..=24);
        let a = noprop::sample_usize_in(ctx, 0..=24);
        let b = noprop::sample_usize_in(ctx, 0..=24);
        let (small, large) = if a <= b { (a, b) } else { (b, a) };

        // A taller keyboard can only shrink the grid.
        assert!(
            tuke::geometry::grid_rows(terminal_rows, small)
                >= tuke::geometry::grid_rows(terminal_rows, large),
            "terminal {terminal_rows}: grid({small}) < grid({large})"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn keyboard_offset_col_centres_within_one_cell() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let centred = std::cell::Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let terminal_cols = noprop::sample_usize_in(ctx, 0..=200);
        let layout_cols = noprop::sample_usize_in(ctx, 0..=200);

        let offset = tuke::geometry::keyboard_offset_col(terminal_cols, layout_cols);
        let right = terminal_cols
            .saturating_sub(layout_cols)
            .saturating_sub(offset);
        let left = offset;

        if layout_cols <= terminal_cols {
            // The layout fits, so the leftover space is split evenly: the two
            // margins differ by at most one cell.
            assert!(
                left.abs_diff(right) <= 1,
                "terminal {terminal_cols}, layout {layout_cols}: left {left}, right {right}"
            );
            if left > 0 {
                centred.set(centred.get() + 1);
            }
        } else {
            // The layout is wider than the screen; there is no room to shift.
            assert_eq!(offset, 0);
        }
        Ok(())
    })?;

    assert!(
        centred.get() > 0,
        "no case exercised a non-zero offset\n{runner}"
    );
    Ok(())
}

#[test]
fn size_round_trips_through_termnix() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let round_tripped = std::cell::Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let rows = noprop::sample_usize_in(ctx, 0..=u16::MAX as usize + 1);
        let cols = noprop::sample_usize_in(ctx, 0..=u16::MAX as usize + 1);
        let size = tuinix::Size { rows, cols };

        match tuke::geometry::to_termnix_size(size) {
            Some(converted) => {
                // Non-zero in-range sizes survive the round trip unchanged.
                assert_ne!(rows, 0);
                assert_ne!(cols, 0);
                assert_eq!(tuke::geometry::from_termnix_size(converted), size);
                round_tripped.set(round_tripped.get() + 1);
            }
            None => {
                // A zero or out-of-range dimension cannot be represented.
                assert!(
                    rows == 0 || cols == 0 || rows > u16::MAX as usize || cols > u16::MAX as usize,
                    "unexpected None for {size:?}"
                );
            }
        }
        Ok(())
    })?;

    assert!(
        round_tripped.get() > 0,
        "no case exercised a representable size\n{runner}"
    );
    Ok(())
}
