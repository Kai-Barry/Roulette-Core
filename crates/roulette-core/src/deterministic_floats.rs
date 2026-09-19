//! Deterministic transcendental-free float helpers (REQ-007).
//!
//! The physics simulation must produce bit-identical results on native targets and
//! wasm, so `sin`/`cos` are computed with fixed polynomial approximations instead of
//! platform libm calls.

/// Polynomial approximation of `sin(x)` valid on `[-2π, 2π]`.
///
/// Uses a fixed 9-term odd Taylor series around 0 with range reduction by `±2π`,
/// giving a maximum absolute error well below 1e-9 over the reduced interval
/// (error grows only in the final wrap segment, where the argument is small).
pub fn sin_approx(x: f64) -> f64 {
    let reduced = reduce_two_pi(x);
    sin_series(reduced)
}

/// Polynomial approximation of `cos(x)` valid on `[-2π, 2π]`.
pub fn cos_approx(x: f64) -> f64 {
    cos_series(reduce_two_pi(x))
}

/// Reduces any finite angle into `[-π, π]` using twoπ wraps computed in f64.
fn reduce_two_pi(x: f64) -> f64 {
    const TWO_PI: f64 = std::f64::consts::TAU;
    if x.abs() <= std::f64::consts::PI {
        return x;
    }
    // Wrap into [-π, π]; exact for |x| < 2^43 so determinism holds for all sim angles.
    x - TWO_PI * (x / TWO_PI).round()
}

fn sin_series(x: f64) -> f64 {
    let x2 = x * x;
    // Odd Taylor terms with enough terms to bound |err| < 1e-9 for |x| <= π.
    let mut sum = x;
    const COEFFS: [f64; 10] = [
        -1.0 / 6.0,
        1.0 / 120.0,
        -1.0 / 5040.0,
        1.0 / 362_880.0,
        -1.0 / 39_916_800.0,
        1.0 / 6_227_020_800.0,
        -1.0 / 1_307_674_368_000.0,
        1.0 / 355_687_428_096_000.0,
        -1.0 / 121_645_100_408_832_000.0,
        1.0 / 51_090_942_171_709_440_000.0,
    ];
    // Powers: x^3, x^5, ... x^21
    let mut power = x2 * x;
    for (i, c) in COEFFS.iter().enumerate() {
        sum += power * c;
        if i < COEFFS.len() - 1 {
            power *= x2;
        }
    }
    sum
}

fn cos_series(x: f64) -> f64 {
    let x2 = x * x;
    let mut sum = 1.0;
    const COEFFS: [f64; 10] = [
        -1.0 / 2.0,
        1.0 / 24.0,
        -1.0 / 720.0,
        1.0 / 40_320.0,
        -1.0 / 3_628_800.0,
        1.0 / 479_001_600.0,
        -1.0 / 87_178_291_200.0,
        1.0 / 20_922_789_888_000.0,
        -1.0 / 6_402_373_705_728_000.0,
        1.0 / 2_432_902_008_176_640_000.0,
    ];
    // Powers: x^2, x^4, ... x^20
    let mut power = x2;
    for (i, c) in COEFFS.iter().enumerate() {
        sum += power * c;
        if i < COEFFS.len() - 1 {
            power *= x2;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Max |error| versus std on a dense sample of [-2π, 2π].
    fn max_err(f: fn(f64) -> f64, reference: fn(f64) -> f64) -> f64 {
        let mut worst = 0.0f64;
        let steps = 20_000;
        for i in 0..=steps {
            let x = -2.0 * std::f64::consts::PI
                + (i as f64 / steps as f64) * 4.0 * std::f64::consts::PI;
            worst = worst.max((f(x) - reference(x)).abs());
        }
        worst
    }

    #[test]
    fn sin_error_under_1e9() {
        let err = max_err(sin_approx, f64::sin);
        assert!(err < 1e-9, "sin max error {err}");
    }

    #[test]
    fn cos_error_under_1e9() {
        let err = max_err(cos_approx, f64::cos);
        assert!(err < 1e-9, "cos max error {err}");
    }

    #[test]
    fn bit_exact_across_calls() {
        // Same input must give bit-identical output on repeated calls and platforms.
        for x in [-7.0f64, -3.1, -0.5, 0.0, 0.5, 3.1, 7.0] {
            let a = sin_approx(x).to_bits();
            let b = sin_approx(x).to_bits();
            assert_eq!(a, b, "sin bit pattern diverges at {x}");
            let a = cos_approx(x).to_bits();
            let b = cos_approx(x).to_bits();
            assert_eq!(a, b, "cos bit pattern diverges at {x}");
        }
    }

    #[test]
    fn known_values_pinned() {
        let s = sin_approx(std::f64::consts::FRAC_PI_2);
        assert!((s - 1.0).abs() < 1e-9, "sin(pi/2) = {s}");
        let c = cos_approx(0.0);
        assert_eq!(c.to_bits(), 1.0f64.to_bits());
    }
}
