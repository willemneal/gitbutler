//! Survival distribution implementations.
//!
//! Each distribution models a different mortality pattern for memory entries:
//! - **Exponential**: constant hazard, memoryless (bugs, task context)
//! - **Weibull**: monotone hazard, increasing or decreasing (architecture, dependencies)
//! - **Bathtub**: high-low-high hazard (conventions)
//! - **Log-normal**: heavy-tailed (cross-repo knowledge)
//!
//! All distributions implement the `SurvivalFunction` trait, providing S(t),
//! f(t), and h(t) -- the survival function, density, and hazard rate.

use crate::types::{DistributionFamily, DistributionParameters, SurvivalDistribution};

/// Core trait for parametric survival distributions.
///
/// S(t) = P(T > t): probability of surviving beyond time t.
/// f(t) = -dS/dt: probability density at time t.
/// h(t) = f(t) / S(t): instantaneous hazard rate at time t.
pub trait SurvivalFunction {
    /// The survival function S(t) = P(T > t).
    fn survival(&self, t: f64) -> f64;

    /// The probability density function f(t).
    fn density(&self, t: f64) -> f64;

    /// The hazard function h(t) = f(t) / S(t).
    fn hazard(&self, t: f64) -> f64 {
        let s = self.survival(t);
        if s <= 1e-15 {
            return f64::INFINITY;
        }
        self.density(t) / s
    }

    /// The cumulative hazard function H(t) = -ln(S(t)).
    fn cumulative_hazard(&self, t: f64) -> f64 {
        let s = self.survival(t);
        if s <= 1e-15 {
            return f64::INFINITY;
        }
        -s.ln()
    }

    /// Median survival time: t such that S(t) = 0.5.
    fn median_survival(&self) -> f64;

    /// Mean survival time (expected value of T).
    fn mean_survival(&self) -> f64;

    /// The distribution family.
    fn family(&self) -> DistributionFamily;
}

/// Exponential distribution: constant hazard rate lambda.
///
/// S(t) = exp(-lambda * t)
/// h(t) = lambda (constant)
///
/// Used for bug/fix memories and task context -- relevance drops
/// at a constant rate, independent of how long the memory has survived.
#[derive(Debug, Clone)]
pub struct Exponential {
    /// Rate parameter (events per day). Must be positive.
    pub lambda: f64,
}

impl Exponential {
    /// Create a new exponential distribution with the given rate.
    pub fn new(lambda: f64) -> anyhow::Result<Self> {
        anyhow::ensure!(lambda > 0.0, "Exponential lambda must be positive, got {lambda}");
        Ok(Self { lambda })
    }
}

impl SurvivalFunction for Exponential {
    fn survival(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 1.0;
        }
        (-self.lambda * t).exp()
    }

    fn density(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        self.lambda * (-self.lambda * t).exp()
    }

    fn hazard(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        self.lambda
    }

    fn cumulative_hazard(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        self.lambda * t
    }

    fn median_survival(&self) -> f64 {
        (2.0_f64).ln() / self.lambda
    }

    fn mean_survival(&self) -> f64 {
        1.0 / self.lambda
    }

    fn family(&self) -> DistributionFamily {
        DistributionFamily::Exponential
    }
}

/// Weibull distribution: monotone hazard rate.
///
/// S(t) = exp(-(t/lambda)^k)
/// h(t) = (k/lambda) * (t/lambda)^(k-1)
///
/// When k > 1, hazard increases over time (aging). When k < 1, hazard
/// decreases (infant mortality). When k = 1, reduces to exponential.
///
/// Used for architectural memories (k ~ 1.5-2.5: slow aging) and
/// dependency memories (k ~ 2.0: moderate aging).
#[derive(Debug, Clone)]
pub struct Weibull {
    /// Shape parameter. k > 1 means increasing hazard.
    pub k: f64,
    /// Scale parameter (days). Characteristic life.
    pub lambda: f64,
}

impl Weibull {
    /// Create a new Weibull distribution.
    pub fn new(k: f64, lambda: f64) -> anyhow::Result<Self> {
        anyhow::ensure!(k > 0.0, "Weibull k must be positive, got {k}");
        anyhow::ensure!(lambda > 0.0, "Weibull lambda must be positive, got {lambda}");
        Ok(Self { k, lambda })
    }
}

impl SurvivalFunction for Weibull {
    fn survival(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 1.0;
        }
        let ratio = t / self.lambda;
        (-ratio.powf(self.k)).exp()
    }

    fn density(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        let ratio = t / self.lambda;
        (self.k / self.lambda) * ratio.powf(self.k - 1.0) * (-ratio.powf(self.k)).exp()
    }

    fn hazard(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return if self.k < 1.0 { f64::INFINITY } else { 0.0 };
        }
        (self.k / self.lambda) * (t / self.lambda).powf(self.k - 1.0)
    }

    fn cumulative_hazard(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        (t / self.lambda).powf(self.k)
    }

    fn median_survival(&self) -> f64 {
        self.lambda * (2.0_f64).ln().powf(1.0 / self.k)
    }

    fn mean_survival(&self) -> f64 {
        // E[T] = lambda * Gamma(1 + 1/k)
        // Using Stirling approximation for the gamma function.
        self.lambda * gamma_approx(1.0 + 1.0 / self.k)
    }

    fn family(&self) -> DistributionFamily {
        DistributionFamily::Weibull
    }
}

/// Bathtub-shaped hazard distribution (additive mixture model).
///
/// h(t) = alpha * exp(-gamma * t) + baseline + beta * t
///
/// This produces the classic bathtub curve:
/// - Early: high hazard from alpha * exp(-gamma * t) (infant mortality)
/// - Middle: low, roughly constant hazard (baseline)
/// - Late: increasing hazard from beta * t (wearout)
///
/// S(t) = exp(-H(t)) where H(t) = integral of h(t).
/// H(t) = (alpha/gamma)(1 - exp(-gamma*t)) + baseline*t + (beta/2)*t^2
///
/// Used for convention memories: initially uncertain, stable when established,
/// increasingly unreliable as teams change.
#[derive(Debug, Clone)]
pub struct Bathtub {
    /// Early-life hazard amplitude.
    pub alpha: f64,
    /// Wearout hazard slope.
    pub beta: f64,
    /// Early-life hazard decay rate.
    pub gamma: f64,
    /// Baseline (constant) hazard component.
    pub baseline: f64,
}

impl Bathtub {
    /// Create a new bathtub distribution.
    pub fn new(alpha: f64, beta: f64, gamma: f64, baseline: f64) -> anyhow::Result<Self> {
        anyhow::ensure!(alpha >= 0.0, "Bathtub alpha must be non-negative, got {alpha}");
        anyhow::ensure!(beta >= 0.0, "Bathtub beta must be non-negative, got {beta}");
        anyhow::ensure!(gamma > 0.0, "Bathtub gamma must be positive, got {gamma}");
        anyhow::ensure!(
            baseline >= 0.0,
            "Bathtub baseline must be non-negative, got {baseline}"
        );
        Ok(Self {
            alpha,
            beta,
            gamma,
            baseline,
        })
    }

    /// Compute the cumulative hazard H(t) analytically.
    fn cumulative_hazard_value(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        let early = (self.alpha / self.gamma) * (1.0 - (-self.gamma * t).exp());
        let constant = self.baseline * t;
        let wearout = (self.beta / 2.0) * t * t;
        early + constant + wearout
    }
}

impl SurvivalFunction for Bathtub {
    fn survival(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 1.0;
        }
        (-self.cumulative_hazard_value(t)).exp()
    }

    fn density(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        self.hazard(t) * self.survival(t)
    }

    fn hazard(&self, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        self.alpha * (-self.gamma * t).exp() + self.baseline + self.beta * t
    }

    fn cumulative_hazard(&self, t: f64) -> f64 {
        self.cumulative_hazard_value(t)
    }

    fn median_survival(&self) -> f64 {
        // Solve H(t) = ln(2) numerically via bisection.
        let target = (2.0_f64).ln();
        bisect_cumulative_hazard(|t| self.cumulative_hazard_value(t), target)
    }

    fn mean_survival(&self) -> f64 {
        // Numerical integration of S(t) from 0 to a large upper bound.
        // E[T] = integral_0^inf S(t) dt.
        numerical_mean_survival(|t| self.survival(t))
    }

    fn family(&self) -> DistributionFamily {
        DistributionFamily::Bathtub
    }
}

/// Log-normal distribution: heavy-tailed survival.
///
/// S(t) = 1 - Phi((ln(t) - mu) / sigma)
/// where Phi is the standard normal CDF.
///
/// Used for cross-repo knowledge, which has a heavy tail: some cross-repo
/// facts are surprisingly durable.
#[derive(Debug, Clone)]
pub struct LogNormal {
    /// Log-mean parameter.
    pub mu: f64,
    /// Log-standard-deviation. Must be positive.
    pub sigma: f64,
}

impl LogNormal {
    /// Create a new log-normal distribution.
    pub fn new(mu: f64, sigma: f64) -> anyhow::Result<Self> {
        anyhow::ensure!(sigma > 0.0, "LogNormal sigma must be positive, got {sigma}");
        Ok(Self { mu, sigma })
    }

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
        let poly =
            t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
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

    /// Compute z = (ln(t) - mu) / sigma for a given t.
    fn z(&self, t: f64) -> f64 {
        (t.ln() - self.mu) / self.sigma
    }
}

impl SurvivalFunction for LogNormal {
    fn survival(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        1.0 - Self::normal_cdf(self.z(t))
    }

    fn density(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        Self::normal_pdf(self.z(t)) / (t * self.sigma)
    }

    fn median_survival(&self) -> f64 {
        // Median of log-normal is exp(mu).
        self.mu.exp()
    }

    fn mean_survival(&self) -> f64 {
        // E[T] = exp(mu + sigma^2 / 2)
        (self.mu + self.sigma * self.sigma / 2.0).exp()
    }

    fn family(&self) -> DistributionFamily {
        DistributionFamily::LogNormal
    }
}

/// Create a boxed `SurvivalFunction` from a `SurvivalDistribution` descriptor.
pub fn from_distribution(dist: &SurvivalDistribution) -> anyhow::Result<Box<dyn SurvivalFunction>> {
    match &dist.parameters {
        DistributionParameters::Exponential { lambda } => {
            Ok(Box::new(Exponential::new(*lambda)?))
        }
        DistributionParameters::Weibull { k, lambda } => {
            Ok(Box::new(Weibull::new(*k, *lambda)?))
        }
        DistributionParameters::Bathtub {
            alpha,
            beta,
            gamma,
            baseline,
        } => Ok(Box::new(Bathtub::new(*alpha, *beta, *gamma, *baseline)?)),
        DistributionParameters::LogNormal { mu, sigma } => {
            Ok(Box::new(LogNormal::new(*mu, *sigma)?))
        }
    }
}

// ---------------------------------------------------------------------------
// Numerical utilities
// ---------------------------------------------------------------------------

/// Lanczos approximation of the gamma function for positive real arguments.
///
/// Accurate to ~15 significant digits for x > 0.5.
/// For x < 0.5 we use the reflection formula.
fn gamma_approx(x: f64) -> f64 {
    if x < 0.5 {
        // Reflection: Gamma(x) = pi / (sin(pi*x) * Gamma(1-x))
        let reflected = gamma_approx(1.0 - x);
        if reflected.abs() < 1e-300 {
            return f64::INFINITY;
        }
        return std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * reflected);
    }

    // Lanczos coefficients (g=7, n=9).
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

/// Bisection solver: find t such that cum_hazard(t) = target.
fn bisect_cumulative_hazard<F: Fn(f64) -> f64>(cum_hazard: F, target: f64) -> f64 {
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    // Expand hi until cumulative hazard exceeds target.
    while cum_hazard(hi) < target && hi < 1e6 {
        hi *= 2.0;
    }
    // Bisect.
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if cum_hazard(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Numerical integration of S(t) for mean survival time.
///
/// Uses composite Simpson's rule over [0, upper_bound].
fn numerical_mean_survival<F: Fn(f64) -> f64>(survival_fn: F) -> f64 {
    // Find a practical upper bound where S(t) ~ 0.
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
        let exp = Exponential::new(0.1).unwrap();
        assert!((exp.survival(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn exponential_median_matches_formula() {
        let exp = Exponential::new(0.1).unwrap();
        let expected = (2.0_f64).ln() / 0.1;
        assert!((exp.median_survival() - expected).abs() < 1e-6);
    }

    #[test]
    fn weibull_reduces_to_exponential_when_k_is_one() {
        let weibull = Weibull::new(1.0, 10.0).unwrap();
        let exp = Exponential::new(1.0 / 10.0).unwrap();
        for &t in &[1.0, 5.0, 10.0, 20.0] {
            assert!((weibull.survival(t) - exp.survival(t)).abs() < 1e-10);
        }
    }

    #[test]
    fn bathtub_survival_is_monotonically_decreasing() {
        let bath = Bathtub::new(0.1, 0.001, 0.5, 0.01).unwrap();
        let mut prev = 1.0;
        for i in 1..100 {
            let t = i as f64;
            let s = bath.survival(t);
            assert!(s <= prev + 1e-10, "S({t}) = {s} > S({prev_t}) = {prev}", prev_t = t - 1.0);
            prev = s;
        }
    }

    #[test]
    fn lognormal_median_is_exp_mu() {
        let ln = LogNormal::new(3.5, 1.2).unwrap();
        let expected = (3.5_f64).exp();
        assert!((ln.median_survival() - expected).abs() < 1e-6);
    }
}
