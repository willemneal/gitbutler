//! Survival distribution implementations.
//!
//! Each distribution models a different mortality pattern for memory entries:
//! - **Exponential**: constant hazard, memoryless (bugs, task context)
//! - **Weibull**: monotone hazard, increasing or decreasing (architecture)
//! - **Bathtub**: high-low-high hazard (conventions)
//! - **Log-normal**: heavy-tailed (cross-repo knowledge)

use crate::types::SurvivalDistribution;

/// Methods on `SurvivalDistribution` for computing S(t), h(t), f(t), and median.
impl SurvivalDistribution {
    /// The survival function S(t) = P(T > t).
    pub fn survival_probability(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 1.0;
        }
        match self {
            Self::Exponential { lambda } => (-lambda * t).exp(),
            Self::Weibull { k, lambda } => {
                let ratio = t / lambda;
                (-ratio.powf(*k)).exp()
            }
            Self::Bathtub {
                alpha,
                beta,
                gamma,
            } => (-cumulative_hazard_bathtub(*alpha, *beta, *gamma, t)).exp(),
            Self::LogNormal { mu, sigma } => {
                if t <= 0.0 {
                    return 1.0;
                }
                1.0 - normal_cdf((t.ln() - mu) / sigma)
            }
        }
    }

    /// The hazard function h(t) = f(t) / S(t).
    pub fn hazard_rate(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        match self {
            Self::Exponential { lambda } => *lambda,
            Self::Weibull { k, lambda } => {
                if t <= 0.0 {
                    return if *k < 1.0 { f64::INFINITY } else { 0.0 };
                }
                (k / lambda) * (t / lambda).powf(k - 1.0)
            }
            Self::Bathtub {
                alpha,
                beta,
                gamma,
            } => alpha * (-gamma * t).exp() + beta * t,
            Self::LogNormal { .. } => {
                let s = self.survival_probability(t);
                if s <= 1e-15 {
                    return f64::INFINITY;
                }
                self.density(t) / s
            }
        }
    }

    /// The probability density function f(t).
    pub fn density(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        match self {
            Self::Exponential { lambda } => {
                if t < 0.0 {
                    return 0.0;
                }
                lambda * (-lambda * t).exp()
            }
            Self::Weibull { k, lambda } => {
                if t <= 0.0 {
                    return 0.0;
                }
                let ratio = t / lambda;
                (k / lambda) * ratio.powf(k - 1.0) * (-ratio.powf(*k)).exp()
            }
            Self::Bathtub {
                alpha,
                beta,
                gamma,
            } => {
                let h = alpha * (-gamma * t).exp() + beta * t;
                let s = (-cumulative_hazard_bathtub(*alpha, *beta, *gamma, t)).exp();
                h * s
            }
            Self::LogNormal { mu, sigma } => {
                if t <= 0.0 {
                    return 0.0;
                }
                let z = (t.ln() - mu) / sigma;
                normal_pdf(z) / (t * sigma)
            }
        }
    }

    /// Median survival time: t such that S(t) = 0.5.
    pub fn median_survival(&self) -> f64 {
        match self {
            Self::Exponential { lambda } => (2.0_f64).ln() / lambda,
            Self::Weibull { k, lambda } => lambda * (2.0_f64).ln().powf(1.0 / k),
            Self::Bathtub {
                alpha,
                beta,
                gamma,
            } => {
                let a = *alpha;
                let b = *beta;
                let g = *gamma;
                let target = (2.0_f64).ln();
                bisect(|t| cumulative_hazard_bathtub(a, b, g, t), target)
            }
            Self::LogNormal { mu, .. } => mu.exp(),
        }
    }

    /// Mean survival time (expected value of T).
    pub fn mean_survival(&self) -> f64 {
        match self {
            Self::Exponential { lambda } => 1.0 / lambda,
            Self::Weibull { k, lambda } => lambda * gamma_approx(1.0 + 1.0 / k),
            Self::Bathtub { .. } | Self::LogNormal { .. } => {
                // Numerical integration of S(t).
                let dist = self.clone();
                numerical_mean(|t| dist.survival_probability(t))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bathtub cumulative hazard
// ---------------------------------------------------------------------------

/// H(t) for the bathtub additive hazard model.
///
/// h(t) = alpha * exp(-gamma * t) + beta * t
/// H(t) = (alpha / gamma)(1 - exp(-gamma * t)) + (beta / 2) * t^2
fn cumulative_hazard_bathtub(alpha: f64, beta: f64, gamma: f64, t: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    let early = (alpha / gamma) * (1.0 - (-gamma * t).exp());
    let wearout = (beta / 2.0) * t * t;
    early + wearout
}

// ---------------------------------------------------------------------------
// Normal CDF / PDF
// ---------------------------------------------------------------------------

/// Standard normal CDF approximation (Abramowitz and Stegun 26.2.17).
fn normal_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let x_abs = x.abs();
    let t = 1.0 / (1.0 + 0.2316419 * x_abs);
    let d = 0.3989422804014327; // 1/sqrt(2*pi)
    let p = d * (-x_abs * x_abs / 2.0).exp();
    let poly = t
        * (0.319381530
            + t * (-0.356563782
                + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    let cdf = 1.0 - p * poly;
    if sign < 0.0 {
        1.0 - cdf
    } else {
        cdf
    }
}

/// Standard normal PDF.
fn normal_pdf(x: f64) -> f64 {
    let inv_sqrt_2pi = 0.3989422804014327;
    inv_sqrt_2pi * (-x * x / 2.0).exp()
}

// ---------------------------------------------------------------------------
// Numerical utilities
// ---------------------------------------------------------------------------

/// Lanczos approximation of the gamma function for positive real arguments.
fn gamma_approx(x: f64) -> f64 {
    if x < 0.5 {
        let reflected = gamma_approx(1.0 - x);
        if reflected.abs() < 1e-300 {
            return f64::INFINITY;
        }
        return std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * reflected);
    }

    let coefficients = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1259.139_216_722_402_9,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];

    let g = 7.0_f64;
    let z = x - 1.0;

    let mut sum = coefficients[0];
    for (i, &c) in coefficients.iter().enumerate().skip(1) {
        sum += c / (z + i as f64);
    }

    let t = z + g + 0.5;
    (2.0 * std::f64::consts::PI).sqrt() * t.powf(z + 0.5) * (-t).exp() * sum
}

/// Bisection solver: find t such that f(t) = target.
fn bisect<F: Fn(f64) -> f64>(f: F, target: f64) -> f64 {
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    while f(hi) < target && hi < 1e6 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Numerical integration of S(t) for mean survival time via Simpson's rule.
fn numerical_mean<F: Fn(f64) -> f64>(survival_fn: F) -> f64 {
    let mut upper = 1.0;
    while survival_fn(upper) > 1e-8 && upper < 1e6 {
        upper *= 2.0;
    }

    let n = 1000_usize;
    let h = upper / n as f64;
    let mut sum = survival_fn(0.0) + survival_fn(upper);

    for i in 1..n {
        let t = i as f64 * h;
        let weight = if i % 2 == 0 { 2.0 } else { 4.0 };
        sum += weight * survival_fn(t);
    }

    sum * h / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_survival_at_zero_is_one() {
        let dist = SurvivalDistribution::Exponential { lambda: 0.1 };
        assert!((dist.survival_probability(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn exponential_median_matches_formula() {
        let dist = SurvivalDistribution::Exponential { lambda: 0.1 };
        let expected = (2.0_f64).ln() / 0.1;
        assert!((dist.median_survival() - expected).abs() < 1e-6);
    }

    #[test]
    fn weibull_reduces_to_exponential_when_k_is_one() {
        let weibull = SurvivalDistribution::Weibull {
            k: 1.0,
            lambda: 10.0,
        };
        let exp = SurvivalDistribution::Exponential {
            lambda: 1.0 / 10.0,
        };
        for &t in &[1.0, 5.0, 10.0, 20.0] {
            assert!((weibull.survival_probability(t) - exp.survival_probability(t)).abs() < 1e-10);
        }
    }

    #[test]
    fn bathtub_survival_is_monotonically_decreasing() {
        let dist = SurvivalDistribution::Bathtub {
            alpha: 0.1,
            beta: 0.001,
            gamma: 0.5,
        };
        let mut prev = 1.0;
        for i in 1..100 {
            let t = i as f64;
            let s = dist.survival_probability(t);
            assert!(s <= prev + 1e-10);
            prev = s;
        }
    }

    #[test]
    fn lognormal_median_is_exp_mu() {
        let dist = SurvivalDistribution::LogNormal {
            mu: 3.5,
            sigma: 1.2,
        };
        let expected = (3.5_f64).exp();
        assert!((dist.median_survival() - expected).abs() < 1e-6);
    }
}
