//! Distribution fitting to observed memory access patterns.
//!
//! Given a memory entry's access history, we estimate the parameters of its
//! survival distribution using maximum likelihood estimation (MLE) for
//! parametric families, and select the best-fitting family using the
//! Akaike Information Criterion (AIC).
//!
//! The fitting process:
//! 1. Compute inter-access intervals from the access history.
//! 2. For each candidate distribution family, estimate parameters via MLE.
//! 3. Compute AIC for each candidate.
//! 4. Select the family with the lowest AIC.
//! 5. Return the fitted `SurvivalDistribution` with goodness-of-fit score.

use crate::types::{
    AccessRecord, DistributionFamily, DistributionParameters, MemoryType, SurvivalDistribution,
};

/// Configuration for the fitting procedure.
#[derive(Debug, Clone)]
pub struct FittingConfig {
    /// Minimum number of access intervals required for fitting.
    /// Below this, we use the memory type's default distribution.
    pub min_intervals: usize,
    /// Families to consider during fitting. If empty, all families are tried.
    pub candidate_families: Vec<DistributionFamily>,
    /// Maximum iterations for iterative MLE procedures.
    pub max_iterations: usize,
    /// Convergence tolerance for iterative procedures.
    pub convergence_tolerance: f64,
}

impl Default for FittingConfig {
    fn default() -> Self {
        Self {
            min_intervals: 3,
            candidate_families: Vec::new(),
            max_iterations: 100,
            convergence_tolerance: 1e-6,
        }
    }
}

/// Result of fitting a single distribution family.
#[derive(Debug, Clone)]
pub struct FitResult {
    pub family: DistributionFamily,
    pub parameters: DistributionParameters,
    /// Negative log-likelihood at the MLE.
    pub neg_log_likelihood: f64,
    /// Number of estimated parameters (for AIC computation).
    pub num_parameters: usize,
    /// AIC = 2*k + 2*NLL where k is num_parameters.
    pub aic: f64,
    /// Goodness-of-fit score in [0, 1], derived from the AIC ranking.
    pub goodness_of_fit: f64,
}

/// Extract inter-access intervals (in days) from an access history.
///
/// Intervals are computed between consecutive accesses, sorted chronologically.
/// The first interval is from creation to first access (if access_history is non-empty
/// and created_at is provided).
pub fn compute_intervals(access_history: &[AccessRecord], created_at: &str) -> Vec<f64> {
    if access_history.is_empty() {
        return Vec::new();
    }

    let mut timestamps: Vec<&str> = Vec::with_capacity(access_history.len() + 1);
    timestamps.push(created_at);
    for record in access_history {
        timestamps.push(&record.timestamp);
    }

    // Sort timestamps lexicographically (ISO 8601 sorts correctly).
    timestamps.sort();

    let mut intervals = Vec::with_capacity(timestamps.len() - 1);
    for window in timestamps.windows(2) {
        let interval = approximate_days_between(window[0], window[1]);
        if interval > 0.0 {
            intervals.push(interval);
        }
    }

    intervals
}

/// Fit the best survival distribution to observed inter-access intervals.
///
/// Returns the best-fitting distribution according to AIC, or a default
/// distribution if there are insufficient data points.
pub fn fit_distribution(
    intervals: &[f64],
    memory_type: MemoryType,
    config: &FittingConfig,
) -> SurvivalDistribution {
    // If insufficient data, return default for memory type.
    if intervals.len() < config.min_intervals {
        return default_distribution(memory_type);
    }

    let families = if config.candidate_families.is_empty() {
        preferred_families(memory_type)
    } else {
        config.candidate_families.clone()
    };

    let mut fits: Vec<FitResult> = families
        .iter()
        .filter_map(|family| fit_family(*family, intervals, config).ok())
        .collect();

    if fits.is_empty() {
        return default_distribution(memory_type);
    }

    // Sort by AIC (lower is better).
    fits.sort_by(|a, b| a.aic.partial_cmp(&b.aic).unwrap_or(std::cmp::Ordering::Equal));

    // Compute goodness-of-fit as relative AIC weight.
    let min_aic = fits[0].aic;
    let total_weight: f64 = fits.iter().map(|f| (-(f.aic - min_aic) / 2.0).exp()).sum();

    for fit in &mut fits {
        fit.goodness_of_fit = (-(fit.aic - min_aic) / 2.0).exp() / total_weight;
    }

    let best = &fits[0];

    SurvivalDistribution {
        family: best.family,
        parameters: best.parameters.clone(),
        fitted_at: String::new(), // Caller should set this.
        goodness_of_fit: best.goodness_of_fit,
    }
}

/// Fit a specific distribution family to the data via MLE.
fn fit_family(
    family: DistributionFamily,
    intervals: &[f64],
    config: &FittingConfig,
) -> anyhow::Result<FitResult> {
    match family {
        DistributionFamily::Exponential => fit_exponential(intervals),
        DistributionFamily::Weibull => fit_weibull(intervals, config),
        DistributionFamily::Bathtub => fit_bathtub(intervals, config),
        DistributionFamily::LogNormal => fit_lognormal(intervals),
    }
}

/// MLE for exponential distribution.
///
/// The MLE of lambda is 1/mean(intervals).
fn fit_exponential(intervals: &[f64]) -> anyhow::Result<FitResult> {
    let n = intervals.len() as f64;
    let mean = intervals.iter().sum::<f64>() / n;
    anyhow::ensure!(mean > 0.0, "Mean interval must be positive");

    let lambda = 1.0 / mean;

    // NLL = n * ln(lambda) - lambda * sum(intervals)... wait, negative of that.
    // Actually for exponential: L = prod(lambda * exp(-lambda*x_i))
    // log L = n*ln(lambda) - lambda*sum(x_i)
    // NLL = -n*ln(lambda) + lambda*sum(x_i)
    let sum: f64 = intervals.iter().sum();
    let nll = -n * lambda.ln() + lambda * sum;

    let aic = 2.0 + 2.0 * nll; // 1 parameter

    Ok(FitResult {
        family: DistributionFamily::Exponential,
        parameters: DistributionParameters::Exponential { lambda },
        neg_log_likelihood: nll,
        num_parameters: 1,
        aic,
        goodness_of_fit: 0.0, // Set later in comparative ranking.
    })
}

/// MLE for Weibull distribution via iterative Newton-Raphson on shape k.
///
/// Given data x_1, ..., x_n, the MLE for Weibull:
/// - lambda_hat = (sum(x_i^k) / n)^(1/k)
/// - k satisfies: 1/k + mean(ln(x_i)) - sum(x_i^k * ln(x_i)) / sum(x_i^k) = 0
fn fit_weibull(intervals: &[f64], config: &FittingConfig) -> anyhow::Result<FitResult> {
    let n = intervals.len() as f64;
    let log_intervals: Vec<f64> = intervals.iter().map(|x| x.ln()).collect();
    let mean_log = log_intervals.iter().sum::<f64>() / n;

    // Initial estimate of k using the method of moments.
    let var_log = log_intervals.iter().map(|x| (x - mean_log).powi(2)).sum::<f64>() / n;
    let mut k = if var_log > 1e-10 {
        // Approximate: for Weibull, Var(ln(X)) ~ pi^2/(6*k^2) + (Euler-Mascheroni)^2-ish
        // Simplified: k ~ pi / (sqrt(6) * sqrt(var_log))
        std::f64::consts::PI / (6.0_f64.sqrt() * var_log.sqrt())
    } else {
        1.0
    };
    k = k.clamp(0.1, 20.0);

    // Newton-Raphson iteration on the profile log-likelihood equation for k.
    for _ in 0..config.max_iterations {
        let xk: Vec<f64> = intervals.iter().map(|x| x.powf(k)).collect();
        let sum_xk: f64 = xk.iter().sum();
        let sum_xk_ln: f64 = intervals
            .iter()
            .zip(xk.iter())
            .map(|(x, xk_i)| xk_i * x.ln())
            .collect::<Vec<f64>>()
            .iter()
            .sum();
        let sum_xk_ln2: f64 = intervals
            .iter()
            .zip(xk.iter())
            .map(|(x, xk_i)| xk_i * x.ln() * x.ln())
            .collect::<Vec<f64>>()
            .iter()
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

    anyhow::ensure!(lambda > 0.0 && lambda.is_finite(), "Weibull lambda estimation failed");
    anyhow::ensure!(k > 0.0 && k.is_finite(), "Weibull k estimation failed");

    // NLL for Weibull.
    let nll: f64 = intervals
        .iter()
        .map(|x| {
            let ratio = x / lambda;
            -(k.ln() - lambda.ln() + (k - 1.0) * (x.ln() - lambda.ln()) - ratio.powf(k))
        })
        .sum();

    let aic = 4.0 + 2.0 * nll; // 2 parameters

    Ok(FitResult {
        family: DistributionFamily::Weibull,
        parameters: DistributionParameters::Weibull { k, lambda },
        neg_log_likelihood: nll,
        num_parameters: 2,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// Simplified MLE for bathtub distribution.
///
/// Because the bathtub distribution has 4 parameters and a complex likelihood
/// surface, we use a heuristic approach: decompose the data into early, middle,
/// and late periods, then estimate each component separately.
fn fit_bathtub(intervals: &[f64], _config: &FittingConfig) -> anyhow::Result<FitResult> {
    let n = intervals.len();
    anyhow::ensure!(n >= 4, "Need at least 4 intervals for bathtub fit");

    let mut sorted = intervals.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Split into thirds: early, middle, late.
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

    // Heuristic parameter estimation.
    let baseline = if middle_mean > 0.0 {
        1.0 / middle_mean
    } else {
        0.01
    };
    let alpha = if early_mean > 0.0 {
        (1.0 / early_mean - baseline).max(0.0)
    } else {
        0.1
    };
    let gamma = if early_mean > 0.0 {
        1.0 / early_mean
    } else {
        0.5
    };
    let beta = if late_mean > 0.0 {
        ((1.0 / late_mean - baseline) / late_mean).max(0.0)
    } else {
        0.001
    };

    // Approximate NLL.
    let nll: f64 = intervals
        .iter()
        .map(|&t| {
            let h = alpha * (-gamma * t).exp() + baseline + beta * t;
            let big_h = (alpha / gamma) * (1.0 - (-gamma * t).exp()) + baseline * t + (beta / 2.0) * t * t;
            -(h.max(1e-15).ln() - big_h)
        })
        .sum();

    let aic = 8.0 + 2.0 * nll; // 4 parameters

    Ok(FitResult {
        family: DistributionFamily::Bathtub,
        parameters: DistributionParameters::Bathtub {
            alpha,
            beta,
            gamma,
            baseline,
        },
        neg_log_likelihood: nll,
        num_parameters: 4,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// MLE for log-normal distribution.
///
/// mu_hat = mean(ln(x_i)), sigma_hat = sqrt(var(ln(x_i)))
fn fit_lognormal(intervals: &[f64]) -> anyhow::Result<FitResult> {
    let n = intervals.len() as f64;
    let log_intervals: Vec<f64> = intervals
        .iter()
        .map(|x| {
            anyhow::ensure!(*x > 0.0, "Log-normal requires positive intervals");
            Ok(x.ln())
        })
        .collect::<anyhow::Result<Vec<f64>>>()?;

    let mu = log_intervals.iter().sum::<f64>() / n;
    let variance = log_intervals.iter().map(|x| (x - mu).powi(2)).sum::<f64>() / n;
    let sigma = variance.sqrt().max(1e-6);

    // NLL for log-normal.
    let nll: f64 = log_intervals
        .iter()
        .zip(intervals.iter())
        .map(|(ln_x, x)| {
            let z = (ln_x - mu) / sigma;
            0.5 * z * z + sigma.ln() + x.ln() + 0.5 * (2.0 * std::f64::consts::PI).ln()
        })
        .sum();

    let aic = 4.0 + 2.0 * nll; // 2 parameters

    Ok(FitResult {
        family: DistributionFamily::LogNormal,
        parameters: DistributionParameters::LogNormal { mu, sigma },
        neg_log_likelihood: nll,
        num_parameters: 2,
        aic,
        goodness_of_fit: 0.0,
    })
}

/// Return the preferred distribution families for a given memory type.
fn preferred_families(memory_type: MemoryType) -> Vec<DistributionFamily> {
    match memory_type {
        MemoryType::Architectural => vec![DistributionFamily::Weibull, DistributionFamily::Exponential],
        MemoryType::BugFix => vec![DistributionFamily::Exponential, DistributionFamily::Weibull],
        MemoryType::Convention => vec![DistributionFamily::Bathtub, DistributionFamily::Weibull],
        MemoryType::Dependency => vec![DistributionFamily::Weibull, DistributionFamily::Exponential],
        MemoryType::TaskContext => vec![DistributionFamily::Exponential],
        MemoryType::CrossRepo => vec![DistributionFamily::LogNormal, DistributionFamily::Weibull],
    }
}

/// Return a default (prior) distribution for a memory type when insufficient data exists.
fn default_distribution(memory_type: MemoryType) -> SurvivalDistribution {
    let (family, parameters) = match memory_type {
        MemoryType::Architectural => (
            DistributionFamily::Weibull,
            DistributionParameters::Weibull {
                k: 1.8,
                lambda: 180.0,
            },
        ),
        MemoryType::BugFix => (
            DistributionFamily::Exponential,
            DistributionParameters::Exponential { lambda: 1.0 / 3.0 },
        ),
        MemoryType::Convention => (
            DistributionFamily::Bathtub,
            DistributionParameters::Bathtub {
                alpha: 0.1,
                beta: 0.001,
                gamma: 0.5,
                baseline: 0.005,
            },
        ),
        MemoryType::Dependency => (
            DistributionFamily::Weibull,
            DistributionParameters::Weibull {
                k: 2.0,
                lambda: 120.0,
            },
        ),
        MemoryType::TaskContext => (
            DistributionFamily::Exponential,
            DistributionParameters::Exponential { lambda: 1.0 / 2.0 },
        ),
        MemoryType::CrossRepo => (
            DistributionFamily::LogNormal,
            DistributionParameters::LogNormal {
                mu: 3.5,
                sigma: 1.2,
            },
        ),
    };

    SurvivalDistribution {
        family,
        parameters,
        fitted_at: String::new(),
        goodness_of_fit: 0.5, // Moderate: using prior, not fitted.
    }
}

/// Approximate the number of days between two ISO 8601 timestamps.
///
/// This is a simplified parser that handles "YYYY-MM-DDThh:mm:ssZ" format.
/// For production use, a proper datetime library would be preferred.
fn approximate_days_between(a: &str, b: &str) -> f64 {
    fn parse_approx_days(s: &str) -> Option<f64> {
        // Parse "YYYY-MM-DDThh:mm:ss" or "YYYY-MM-DD"
        let parts: Vec<&str> = s.split('T').collect();
        let date_parts: Vec<&str> = parts.first()?.split('-').collect();
        if date_parts.len() < 3 {
            return None;
        }
        let year: f64 = date_parts[0].parse().ok()?;
        let month: f64 = date_parts[1].parse().ok()?;
        let day: f64 = date_parts[2].parse().ok()?;
        // Approximate: 365.25 days/year, 30.44 days/month.
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
        // Data generated from Exp(lambda=0.5), so mean ~ 2.0.
        let intervals = vec![1.5, 2.3, 1.8, 2.1, 2.0, 1.7, 2.4, 1.9, 2.2, 2.0];
        let result = fit_exponential(&intervals).unwrap();
        if let DistributionParameters::Exponential { lambda } = result.parameters {
            let expected = 1.0 / (intervals.iter().sum::<f64>() / intervals.len() as f64);
            assert!((lambda - expected).abs() < 1e-6);
        } else {
            panic!("Expected exponential parameters");
        }
    }

    #[test]
    fn default_distributions_match_memory_types() {
        let arch = default_distribution(MemoryType::Architectural);
        assert_eq!(arch.family, DistributionFamily::Weibull);

        let bug = default_distribution(MemoryType::BugFix);
        assert_eq!(bug.family, DistributionFamily::Exponential);

        let conv = default_distribution(MemoryType::Convention);
        assert_eq!(conv.family, DistributionFamily::Bathtub);
    }
}
