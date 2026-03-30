//! Distribution fitting to observed memory access patterns.
//!
//! Given a memory entry's access history (as inter-access intervals in days),
//! we estimate the parameters of its survival distribution using maximum
//! likelihood estimation (MLE) and select the best-fitting family using the
//! Akaike Information Criterion (AIC).

use crate::types::SurvivalDistribution;

/// Configuration for the fitting procedure.
#[derive(Debug, Clone)]
pub struct FittingConfig {
    /// Minimum number of access intervals required for fitting.
    /// Below this, we return a default distribution.
    pub min_intervals: usize,
    /// Maximum iterations for iterative MLE procedures.
    pub max_iterations: usize,
    /// Convergence tolerance for iterative procedures.
    pub convergence_tolerance: f64,
}

impl Default for FittingConfig {
    fn default() -> Self {
        Self {
            min_intervals: 3,
            max_iterations: 100,
            convergence_tolerance: 1e-6,
        }
    }
}

/// Result of fitting a single distribution family.
#[derive(Debug, Clone)]
pub struct FitResult {
    /// The fitted distribution.
    pub distribution: SurvivalDistribution,
    /// Negative log-likelihood at the MLE.
    pub neg_log_likelihood: f64,
    /// Number of estimated parameters (for AIC computation).
    pub num_parameters: usize,
    /// AIC = 2*k + 2*NLL.
    pub aic: f64,
    /// Goodness-of-fit score in [0, 1], derived from AIC ranking.
    pub goodness_of_fit: f64,
}

/// Categories of memory for choosing default priors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCategory {
    /// Architectural knowledge -- slow aging.
    Architectural,
    /// Bug/fix memories -- constant hazard.
    BugFix,
    /// Convention memories -- bathtub curve.
    Convention,
    /// Dependency memories -- moderate aging.
    Dependency,
    /// Task context -- fast expiration.
    TaskContext,
    /// Cross-repo knowledge -- heavy-tailed.
    CrossRepo,
}

/// Fit the best survival distribution to observed inter-access intervals.
///
/// Tries all four distribution families, selects the one with the lowest AIC.
/// If there are fewer intervals than `config.min_intervals`, returns the
/// default prior for the given memory category.
pub fn fit_distribution(
    intervals: &[f64],
    category: MemoryCategory,
    config: &FittingConfig,
) -> FitResult {
    if intervals.len() < config.min_intervals {
        let dist = default_prior(category);
        return FitResult {
            distribution: dist,
            neg_log_likelihood: 0.0,
            num_parameters: 0,
            aic: 0.0,
            goodness_of_fit: 0.5,
        };
    }

    let candidates: Vec<Option<FitResult>> = vec![
        fit_exponential(intervals),
        fit_weibull(intervals, config),
        fit_bathtub(intervals),
        fit_lognormal(intervals),
    ];

    let mut fits: Vec<FitResult> = candidates.into_iter().flatten().collect();

    if fits.is_empty() {
        let dist = default_prior(category);
        return FitResult {
            distribution: dist,
            neg_log_likelihood: 0.0,
            num_parameters: 0,
            aic: 0.0,
            goodness_of_fit: 0.5,
        };
    }

    fits.sort_by(|a, b| a.aic.partial_cmp(&b.aic).unwrap_or(std::cmp::Ordering::Equal));

    // Compute AIC weights.
    let min_aic = fits[0].aic;
    let total_weight: f64 = fits.iter().map(|f| (-(f.aic - min_aic) / 2.0).exp()).sum();
    for fit in &mut fits {
        fit.goodness_of_fit = (-(fit.aic - min_aic) / 2.0).exp() / total_weight;
    }

    fits.into_iter().next().unwrap()
}

/// Default prior distribution for a memory category.
pub fn default_prior(category: MemoryCategory) -> SurvivalDistribution {
    match category {
        MemoryCategory::Architectural => SurvivalDistribution::Weibull {
            k: 1.8,
            lambda: 180.0,
        },
        MemoryCategory::BugFix => SurvivalDistribution::Exponential {
            lambda: 1.0 / 3.0,
        },
        MemoryCategory::Convention => SurvivalDistribution::Bathtub {
            alpha: 0.1,
            beta: 0.001,
            gamma: 0.5,
        },
        MemoryCategory::Dependency => SurvivalDistribution::Weibull {
            k: 2.0,
            lambda: 120.0,
        },
        MemoryCategory::TaskContext => SurvivalDistribution::Exponential { lambda: 0.5 },
        MemoryCategory::CrossRepo => SurvivalDistribution::LogNormal {
            mu: 3.5,
            sigma: 1.2,
        },
    }
}

// ---------------------------------------------------------------------------
// MLE fitting for each family
// ---------------------------------------------------------------------------

/// MLE for exponential: lambda_hat = 1 / mean(intervals).
fn fit_exponential(intervals: &[f64]) -> Option<FitResult> {
    let n = intervals.len() as f64;
    let mean = intervals.iter().sum::<f64>() / n;
    if mean <= 0.0 {
        return None;
    }
    let lambda = 1.0 / mean;

    let sum: f64 = intervals.iter().sum();
    let nll = -n * lambda.ln() + lambda * sum;
    let aic = 2.0 + 2.0 * nll; // 1 parameter

    Some(FitResult {
        distribution: SurvivalDistribution::Exponential { lambda },
        neg_log_likelihood: nll,
        num_parameters: 1,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// MLE for Weibull via Newton-Raphson on the shape parameter k.
fn fit_weibull(intervals: &[f64], config: &FittingConfig) -> Option<FitResult> {
    let n = intervals.len() as f64;
    let log_intervals: Vec<f64> = intervals.iter().map(|x| x.ln()).collect();
    let mean_log = log_intervals.iter().sum::<f64>() / n;

    let var_log = log_intervals
        .iter()
        .map(|x| (x - mean_log).powi(2))
        .sum::<f64>()
        / n;

    let mut k = if var_log > 1e-10 {
        std::f64::consts::PI / (6.0_f64.sqrt() * var_log.sqrt())
    } else {
        1.0
    };
    k = k.clamp(0.1, 20.0);

    for _ in 0..config.max_iterations {
        let xk: Vec<f64> = intervals.iter().map(|x| x.powf(k)).collect();
        let sum_xk: f64 = xk.iter().sum();
        let sum_xk_ln: f64 = intervals
            .iter()
            .zip(xk.iter())
            .map(|(x, xk_i)| xk_i * x.ln())
            .sum();
        let sum_xk_ln2: f64 = intervals
            .iter()
            .zip(xk.iter())
            .map(|(x, xk_i)| xk_i * x.ln() * x.ln())
            .sum();

        if sum_xk.abs() < 1e-300 {
            break;
        }

        let g = 1.0 / k + mean_log - sum_xk_ln / sum_xk;
        let g_prime =
            -1.0 / (k * k) - (sum_xk_ln2 * sum_xk - sum_xk_ln * sum_xk_ln) / (sum_xk * sum_xk);

        if g_prime.abs() < 1e-300 {
            break;
        }

        let delta = g / g_prime;
        k -= delta;
        k = k.clamp(0.01, 50.0);

        if delta.abs() < config.convergence_tolerance {
            break;
        }
    }

    let xk: Vec<f64> = intervals.iter().map(|x| x.powf(k)).collect();
    let sum_xk: f64 = xk.iter().sum();
    let lambda = (sum_xk / n).powf(1.0 / k);

    if !(lambda > 0.0 && lambda.is_finite() && k > 0.0 && k.is_finite()) {
        return None;
    }

    let nll: f64 = intervals
        .iter()
        .map(|x| {
            let ratio = x / lambda;
            -(k.ln() - lambda.ln() + (k - 1.0) * (x.ln() - lambda.ln()) - ratio.powf(k))
        })
        .sum();

    let aic = 4.0 + 2.0 * nll; // 2 parameters

    Some(FitResult {
        distribution: SurvivalDistribution::Weibull { k, lambda },
        neg_log_likelihood: nll,
        num_parameters: 2,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// Heuristic MLE for bathtub distribution.
fn fit_bathtub(intervals: &[f64]) -> Option<FitResult> {
    let n = intervals.len();
    if n < 4 {
        return None;
    }

    let mut sorted = intervals.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let third = n / 3;
    let early = &sorted[..third.max(1)];
    let middle = &sorted[third..n.saturating_sub(third)];
    let late = &sorted[n.saturating_sub(third)..];

    let early_mean = early.iter().sum::<f64>() / early.len() as f64;
    let middle_mean = if middle.is_empty() {
        early_mean
    } else {
        middle.iter().sum::<f64>() / middle.len() as f64
    };
    let late_mean = if late.is_empty() {
        middle_mean
    } else {
        late.iter().sum::<f64>() / late.len() as f64
    };

    let alpha = if early_mean > 0.0 {
        (1.0 / early_mean).max(0.01)
    } else {
        0.1
    };
    let gamma = if early_mean > 0.0 {
        1.0 / early_mean
    } else {
        0.5
    };
    let beta = if late_mean > 0.0 {
        ((1.0 / late_mean) / late_mean).max(0.0)
    } else {
        0.001
    };

    let nll: f64 = intervals
        .iter()
        .map(|&t| {
            let h = alpha * (-gamma * t).exp() + beta * t;
            let big_h =
                (alpha / gamma) * (1.0 - (-gamma * t).exp()) + (beta / 2.0) * t * t;
            -(h.max(1e-15).ln() - big_h)
        })
        .sum();

    let aic = 6.0 + 2.0 * nll; // 3 parameters (alpha, beta, gamma)

    Some(FitResult {
        distribution: SurvivalDistribution::Bathtub {
            alpha,
            beta,
            gamma,
        },
        neg_log_likelihood: nll,
        num_parameters: 3,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// MLE for log-normal: mu_hat = mean(ln(x)), sigma_hat = std(ln(x)).
fn fit_lognormal(intervals: &[f64]) -> Option<FitResult> {
    if intervals.iter().any(|&x| x <= 0.0) {
        return None;
    }

    let n = intervals.len() as f64;
    let log_intervals: Vec<f64> = intervals.iter().map(|x| x.ln()).collect();
    let mu = log_intervals.iter().sum::<f64>() / n;
    let variance = log_intervals
        .iter()
        .map(|x| (x - mu).powi(2))
        .sum::<f64>()
        / n;
    let sigma = variance.sqrt().max(1e-6);

    let nll: f64 = log_intervals
        .iter()
        .zip(intervals.iter())
        .map(|(ln_x, x)| {
            let z = (ln_x - mu) / sigma;
            0.5 * z * z + sigma.ln() + x.ln() + 0.5 * (2.0 * std::f64::consts::PI).ln()
        })
        .sum();

    let aic = 4.0 + 2.0 * nll; // 2 parameters

    Some(FitResult {
        distribution: SurvivalDistribution::LogNormal { mu, sigma },
        neg_log_likelihood: nll,
        num_parameters: 2,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// Compute inter-access intervals (in days) from a sorted list of timestamps.
///
/// Each timestamp is an ISO-8601 string. Intervals are measured between
/// consecutive timestamps.
pub fn compute_intervals(timestamps: &[&str]) -> Vec<f64> {
    if timestamps.len() < 2 {
        return Vec::new();
    }

    let mut sorted: Vec<&str> = timestamps.to_vec();
    sorted.sort();

    let mut intervals = Vec::with_capacity(sorted.len() - 1);
    for window in sorted.windows(2) {
        let interval = approximate_days_between(window[0], window[1]);
        if interval > 0.0 {
            intervals.push(interval);
        }
    }
    intervals
}

/// Approximate the number of days between two ISO-8601 timestamps.
fn approximate_days_between(a: &str, b: &str) -> f64 {
    fn parse_approx_days(s: &str) -> Option<f64> {
        let parts: Vec<&str> = s.split('T').collect();
        let date_parts: Vec<&str> = parts.first()?.split('-').collect();
        if date_parts.len() < 3 {
            return None;
        }
        let year: f64 = date_parts[0].parse().ok()?;
        let month: f64 = date_parts[1].parse().ok()?;
        let day: f64 = date_parts[2].parse().ok()?;
        let total = year * 365.25 + month * 30.44 + day;

        if parts.len() > 1 {
            let time_str = parts[1].trim_end_matches('Z');
            let time_parts: Vec<&str> = time_str.split(':').collect();
            if time_parts.len() >= 2 {
                let hour: f64 = time_parts[0].parse().unwrap_or(0.0);
                let minute: f64 = time_parts[1].parse().unwrap_or(0.0);
                return Some(total + hour / 24.0 + minute / 1440.0);
            }
        }
        Some(total)
    }

    match (parse_approx_days(a), parse_approx_days(b)) {
        (Some(da), Some(db)) => (db - da).max(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_mle_recovers_rate() {
        let intervals = vec![1.5, 2.3, 1.8, 2.1, 2.0, 1.7, 2.4, 1.9, 2.2, 2.0];
        let result = fit_exponential(&intervals).unwrap();
        if let SurvivalDistribution::Exponential { lambda } = result.distribution {
            let expected = 1.0 / (intervals.iter().sum::<f64>() / intervals.len() as f64);
            assert!((lambda - expected).abs() < 1e-6);
        } else {
            panic!("Expected exponential distribution");
        }
    }

    #[test]
    fn default_priors_match_categories() {
        assert!(matches!(
            default_prior(MemoryCategory::Architectural),
            SurvivalDistribution::Weibull { .. }
        ));
        assert!(matches!(
            default_prior(MemoryCategory::BugFix),
            SurvivalDistribution::Exponential { .. }
        ));
        assert!(matches!(
            default_prior(MemoryCategory::Convention),
            SurvivalDistribution::Bathtub { .. }
        ));
    }
}
