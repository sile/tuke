//! Properties of the terminal/keyboard geometry helpers.

#[test]
fn size_round_trips_through_termnix() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let round_tripped = std::cell::Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let rows = noprop::sample_usize_in(ctx, 0..=u16::MAX as usize + 1);
        let cols = noprop::sample_usize_in(ctx, 0..=u16::MAX as usize + 1);
        let size = tuinix::Size { rows, cols };

        match tuke::to_termnix_size(size) {
            Some(converted) => {
                // Non-zero in-range sizes survive the round trip unchanged.
                assert_ne!(rows, 0);
                assert_ne!(cols, 0);
                assert_eq!(tuke::from_termnix_size(converted), size);
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
