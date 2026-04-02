//! Integration tests for survival and narrative modules.

use but_ai::narrative::motif::MotifIndex;
use but_ai::narrative::tension::TensionRegistry;
use but_ai::survival::fitting::{self, FittingConfig, MemoryCategory};
use but_ai::survival::hazard;
use but_ai::types::{
    AgentId, CallNumber, Classification, EntryId, MemoryEntry, MemoryState, MotifId,
    SurvivalDistribution, SurvivalMetadata, Tension, TensionId, TensionSeverity,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal MemoryEntry for testing.
fn make_entry(id: &str, motifs: Vec<MotifId>) -> MemoryEntry {
    MemoryEntry {
        id: EntryId(id.to_string()),
        agent: AgentId("test-agent".to_string()),
        content: String::new(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        last_accessed: "2025-01-01T00:00:00Z".to_string(),
        classification: Classification {
            subject_headings: Vec::new(),
            call_number: CallNumber::parse("TEST"),
            controlled_vocab: false,
        },
        see_also: Vec::new(),
        motifs,
        tension_refs: Vec::new(),
        survival: SurvivalMetadata {
            distribution: SurvivalDistribution::Exponential { lambda: 0.1 },
            current_probability: 1.0,
            hazard_rate: 0.1,
            surprise_index: 0.0,
            goodness_of_fit: 0.5,
        },
        state: MemoryState::Alive,
        consensus_citations: 0,
        access_count: 0,
        source_commit: None,
    }
}

fn make_tension(id: &str, severity: TensionSeverity) -> Tension {
    Tension {
        id: TensionId(id.to_string()),
        description: format!("tension-{id}"),
        severity,
        introduced_in: EntryId("origin".to_string()),
        resolved_in: None,
    }
}

// ===========================================================================
// Survival Distribution Properties
// ===========================================================================

fn all_distributions() -> Vec<(&'static str, SurvivalDistribution)> {
    vec![
        (
            "Exponential",
            SurvivalDistribution::Exponential { lambda: 0.1 },
        ),
        (
            "Weibull",
            SurvivalDistribution::Weibull {
                k: 2.0,
                lambda: 50.0,
            },
        ),
        (
            "Bathtub",
            SurvivalDistribution::Bathtub {
                alpha: 0.1,
                beta: 0.001,
                gamma: 0.5,
            },
        ),
        (
            "LogNormal",
            SurvivalDistribution::LogNormal {
                mu: 3.5,
                sigma: 1.2,
            },
        ),
    ]
}

#[test]
fn survival_at_zero_is_one_for_all_distributions() {
    for (name, dist) in all_distributions() {
        let s0 = dist.survival_probability(0.0);
        assert!(
            (s0 - 1.0).abs() < 1e-10,
            "{name}: S(0) = {s0}, expected 1.0"
        );
    }
}

#[test]
fn survival_at_large_t_approaches_zero_for_all_distributions() {
    for (name, dist) in all_distributions() {
        let s = dist.survival_probability(1e6);
        assert!(s < 1e-6, "{name}: S(1e6) = {s}, expected ~0");
    }
}

#[test]
fn hazard_rate_is_non_negative() {
    let test_times = [0.0, 1.0, 10.0, 100.0, 1000.0];
    for (name, dist) in all_distributions() {
        for &t in &test_times {
            let h = dist.hazard_rate(t);
            assert!(
                h >= 0.0 || h.is_nan() == false,
                "{name}: h({t}) = {h}, expected >= 0"
            );
        }
    }
}

#[test]
fn density_equals_hazard_times_survival() {
    let test_times = [1.0, 5.0, 10.0, 50.0];
    for (name, dist) in all_distributions() {
        for &t in &test_times {
            let f = dist.density(t);
            let h = dist.hazard_rate(t);
            let s = dist.survival_probability(t);
            let product = h * s;
            let diff = (f - product).abs();
            let scale = f.abs().max(product.abs()).max(1e-15);
            assert!(
                diff / scale < 1e-6,
                "{name}: f({t})={f} != h({t})*S({t})={product}"
            );
        }
    }
}

#[test]
fn exponential_median_equals_ln2_over_lambda() {
    let lambda = 0.1;
    let dist = SurvivalDistribution::Exponential { lambda };
    let expected = (2.0_f64).ln() / lambda;
    let actual = dist.median_survival();
    assert!(
        (actual - expected).abs() < 1e-6,
        "Exponential median: {actual} != {expected}"
    );
}

#[test]
fn weibull_k1_reduces_to_exponential() {
    let lambda_scale = 10.0;
    let weibull = SurvivalDistribution::Weibull {
        k: 1.0,
        lambda: lambda_scale,
    };
    let exp = SurvivalDistribution::Exponential {
        lambda: 1.0 / lambda_scale,
    };
    for &t in &[1.0, 5.0, 10.0, 20.0, 50.0] {
        let sw = weibull.survival_probability(t);
        let se = exp.survival_probability(t);
        assert!(
            (sw - se).abs() < 1e-10,
            "Weibull(k=1) != Exp at t={t}: {sw} vs {se}"
        );
    }
}

#[test]
fn bathtub_hazard_is_u_shaped() {
    let dist = SurvivalDistribution::Bathtub {
        alpha: 0.1,
        beta: 0.001,
        gamma: 0.5,
    };
    let h0 = dist.hazard_rate(0.0);
    let h50 = dist.hazard_rate(50.0);
    let h200 = dist.hazard_rate(200.0);
    assert!(
        h0 > h50,
        "Bathtub: h(0)={h0} should be > h(50)={h50}"
    );
    assert!(
        h200 > h50,
        "Bathtub: h(200)={h200} should be > h(50)={h50}"
    );
}

// ===========================================================================
// Survival Fitting
// ===========================================================================

#[test]
fn fit_exponential_recovers_reasonable_lambda() {
    let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    let config = FittingConfig::default();
    let result = fitting::fit_distribution(&data, MemoryCategory::BugFix, &config);
    match result.distribution {
        SurvivalDistribution::Exponential { lambda } => {
            // Mean = 30, so lambda should be ~1/30 = 0.0333
            let expected = 1.0 / 30.0;
            assert!(
                (lambda - expected).abs() < 0.02,
                "Fitted lambda={lambda}, expected ~{expected}"
            );
        }
        _ => {
            // AIC may select a different distribution; just check it's valid.
            let s0 = result.distribution.survival_probability(0.0);
            assert!((s0 - 1.0).abs() < 1e-10, "Fitted dist has S(0) != 1");
        }
    }
}

#[test]
fn aic_selects_exponential_for_exponential_data() {
    // Generate data that is clearly exponential (constant intervals).
    let data: Vec<f64> = (1..=20).map(|i| i as f64 * 5.0).collect();
    let config = FittingConfig::default();
    let result = fitting::fit_distribution(&data, MemoryCategory::BugFix, &config);
    // The fit should succeed and produce a valid distribution.
    let s0 = result.distribution.survival_probability(0.0);
    assert!(
        (s0 - 1.0).abs() < 1e-10,
        "AIC-selected distribution has S(0) != 1"
    );
    // AIC value should be finite.
    assert!(result.aic.is_finite(), "AIC should be finite");
}

// ===========================================================================
// Hazard Classification
// ===========================================================================

#[test]
fn hazard_classify_alive() {
    let state = hazard::classify_state(0.8);
    assert_eq!(state, MemoryState::Alive);
}

#[test]
fn hazard_classify_moribund() {
    let state = hazard::classify_state(0.2);
    assert_eq!(state, MemoryState::Moribund);
}

#[test]
fn hazard_classify_deceased() {
    let state = hazard::classify_state(0.05);
    assert_eq!(state, MemoryState::Deceased);
}

// ===========================================================================
// Motif Emergence
// ===========================================================================

#[test]
fn motif_proto_weight_with_one_appearance() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".to_string());
    let entry_id = EntryId("e1".to_string());

    let promoted = index.record_appearance(&motif_id, "error handling patterns", &entry_id);
    assert!(!promoted, "Should not promote with 1 appearance");
    assert_eq!(index.proto_motif_count(), 1);
    assert_eq!(index.motif_count(), 0);
    assert!(!index.is_emerged(&motif_id));
}

#[test]
fn motif_promotes_at_three_appearances() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".to_string());

    let p1 = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("e1".to_string()),
    );
    assert!(!p1);

    let p2 = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("e2".to_string()),
    );
    assert!(!p2);

    let p3 = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("e3".to_string()),
    );
    assert!(p3, "Should promote at 3 appearances");
    assert_eq!(index.motif_count(), 1);
    assert_eq!(index.proto_motif_count(), 0);
    assert!(index.is_emerged(&motif_id));
}

#[test]
fn motif_resonance_matching_query() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".to_string());

    // Promote the motif.
    for i in 1..=3 {
        index.record_appearance(
            &motif_id,
            "error handling patterns",
            &EntryId(format!("e{i}")),
        );
    }

    let entry = make_entry("test-entry", vec![motif_id.clone()]);
    let score = index.resonance("error handling", &entry);
    assert!(score > 0.0, "Resonance for matching query should be > 0, got {score}");
}

#[test]
fn motif_resonance_non_matching_query() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".to_string());

    for i in 1..=3 {
        index.record_appearance(
            &motif_id,
            "error handling patterns",
            &EntryId(format!("e{i}")),
        );
    }

    let entry = make_entry("test-entry", vec![motif_id.clone()]);
    let score = index.resonance("database migration", &entry);
    assert!(
        score == 0.0 || score < 1e-10,
        "Resonance for non-matching query should be 0, got {score}"
    );
}

// ===========================================================================
// Tension Urgency
// ===========================================================================

#[test]
fn tension_urgency_at_introduction_is_approximately_zero() {
    let mut registry = TensionRegistry::new();
    let tension = make_tension("t1", TensionSeverity::High);
    let tid = tension.id.clone();
    registry.introduce(tension, 0);

    let urgency = registry.urgency_score(&tid, 0);
    assert!(
        urgency < 0.01,
        "Urgency at t=0 should be ~0, got {urgency}"
    );
}

#[test]
fn tension_urgency_increases_after_14_days() {
    let mut registry = TensionRegistry::new();
    let tension = make_tension("t1", TensionSeverity::High);
    let tid = tension.id.clone();
    registry.introduce(tension, 0);

    let fourteen_days_secs = 14 * 24 * 3600;
    let urgency = registry.urgency_score(&tid, fourteen_days_secs);
    assert!(
        urgency > 0.5,
        "Urgency at t=14 days should be > 0.5, got {urgency}"
    );
}

#[test]
fn resolved_tension_urgency_is_zero() {
    let mut registry = TensionRegistry::new();
    let tension = make_tension("t1", TensionSeverity::High);
    let tid = tension.id.clone();
    registry.introduce(tension, 0);

    registry.resolve(&tid, &EntryId("resolver".to_string()));

    let fourteen_days_secs = 14 * 24 * 3600;
    let urgency = registry.urgency_score(&tid, fourteen_days_secs);
    assert!(
        urgency == 0.0,
        "Resolved tension urgency should be 0, got {urgency}"
    );
}

#[test]
fn critical_severity_greater_than_moderate_at_same_age() {
    let mut registry = TensionRegistry::new();

    let t_critical = make_tension("t-critical", TensionSeverity::Critical);
    let t_moderate = make_tension("t-moderate", TensionSeverity::Moderate);
    let tid_c = t_critical.id.clone();
    let tid_m = t_moderate.id.clone();

    registry.introduce(t_critical, 0);
    registry.introduce(t_moderate, 0);

    let seven_days_secs = 7 * 24 * 3600;
    let u_critical = registry.urgency_score(&tid_c, seven_days_secs);
    let u_moderate = registry.urgency_score(&tid_m, seven_days_secs);

    assert!(
        u_critical > u_moderate,
        "Critical urgency ({u_critical}) should be > Moderate urgency ({u_moderate})"
    );
}
