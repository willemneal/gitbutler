//! Controlled vocabulary management.
//!
//! Without a controlled vocabulary, the same concept gets classified under
//! "authentication", "auth", "login", "sign-in", and "access control".
//! Searches for any one term miss the others.
//!
//! The controlled vocabulary maps variant terms to canonical terms,
//! ensuring that classification and retrieval use consistent language.

use crate::types::{ControlledVocabulary, VocabularyMapping};
use std::collections::HashMap;

/// A live vocabulary index built from a [`ControlledVocabulary`].
///
/// Provides fast lookup from variant -> canonical and reverse lookup
/// from canonical -> all variants.
#[derive(Debug, Clone)]
pub struct VocabularyIndex {
    /// variant (lowercased) -> canonical (lowercased).
    variant_to_canonical: HashMap<String, String>,
    /// canonical (lowercased) -> list of variants.
    canonical_to_variants: HashMap<String, Vec<String>>,
    /// Maximum number of canonical terms allowed.
    max_headings: usize,
}

impl VocabularyIndex {
    /// Build an index from a [`ControlledVocabulary`].
    pub fn from_vocabulary(vocab: &ControlledVocabulary, max_headings: usize) -> Self {
        let mut variant_to_canonical = HashMap::new();
        let mut canonical_to_variants: HashMap<String, Vec<String>> = HashMap::new();

        for mapping in &vocab.mappings {
            let variant = mapping.variant.to_lowercase();
            let canonical = mapping.canonical.to_lowercase();

            variant_to_canonical.insert(variant.clone(), canonical.clone());
            canonical_to_variants
                .entry(canonical)
                .or_default()
                .push(variant);
        }

        Self {
            variant_to_canonical,
            canonical_to_variants,
            max_headings,
        }
    }

    /// Create an empty index.
    pub fn empty(max_headings: usize) -> Self {
        Self {
            variant_to_canonical: HashMap::new(),
            canonical_to_variants: HashMap::new(),
            max_headings,
        }
    }

    /// Normalize a term to its canonical form. If the term is already
    /// canonical or unknown, returns it as-is (lowercased).
    pub fn normalize(&self, term: &str) -> String {
        let lower = term.to_lowercase();
        self.variant_to_canonical
            .get(&lower)
            .cloned()
            .unwrap_or(lower)
    }

    /// Normalize a list of subject headings, deduplicating after normalization.
    pub fn normalize_subjects(&self, subjects: &[String]) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for subject in subjects {
            let canonical = self.normalize(subject);
            if seen.insert(canonical.clone()) {
                result.push(canonical);
            }
        }
        result
    }

    /// Add a new mapping. Returns `false` if the maximum headings limit
    /// would be exceeded by adding a new canonical term.
    pub fn add_mapping(&mut self, variant: &str, canonical: &str) -> bool {
        let variant_lower = variant.to_lowercase();
        let canonical_lower = canonical.to_lowercase();

        // Check if this would create a new canonical term.
        if !self.canonical_to_variants.contains_key(&canonical_lower)
            && self.canonical_to_variants.len() >= self.max_headings
        {
            tracing::warn!(
                canonical = canonical_lower,
                max = self.max_headings,
                "controlled vocabulary limit reached"
            );
            return false;
        }

        self.variant_to_canonical
            .insert(variant_lower.clone(), canonical_lower.clone());
        self.canonical_to_variants
            .entry(canonical_lower)
            .or_default()
            .push(variant_lower);
        true
    }

    /// Remove a canonical term and all its variants.
    pub fn remove_canonical(&mut self, canonical: &str) {
        let canonical_lower = canonical.to_lowercase();
        if let Some(variants) = self.canonical_to_variants.remove(&canonical_lower) {
            for variant in variants {
                self.variant_to_canonical.remove(&variant);
            }
        }
    }

    /// Get all variants for a canonical term.
    pub fn get_variants(&self, canonical: &str) -> &[String] {
        self.canonical_to_variants
            .get(&canonical.to_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all canonical terms.
    pub fn all_canonical_terms(&self) -> Vec<&str> {
        self.canonical_to_variants
            .keys()
            .map(|s| s.as_str())
            .collect()
    }

    /// Number of canonical terms.
    pub fn canonical_count(&self) -> usize {
        self.canonical_to_variants.len()
    }

    /// Expand a query: given a list of search terms, add all known variants
    /// and canonical forms to broaden the search.
    pub fn expand_query(&self, terms: &[String]) -> Vec<String> {
        let mut expanded = std::collections::HashSet::new();

        for term in terms {
            let lower = term.to_lowercase();
            expanded.insert(lower.clone());

            // If it's a variant, add the canonical form.
            if let Some(canonical) = self.variant_to_canonical.get(&lower) {
                expanded.insert(canonical.clone());
                // Add all sibling variants of the same canonical term.
                if let Some(variants) = self.canonical_to_variants.get(canonical) {
                    for v in variants {
                        expanded.insert(v.clone());
                    }
                }
            }

            // If it's a canonical term, add all its variants.
            if let Some(variants) = self.canonical_to_variants.get(&lower) {
                for v in variants {
                    expanded.insert(v.clone());
                }
            }
        }

        expanded.into_iter().collect()
    }

    /// Export the index back to a [`ControlledVocabulary`].
    pub fn to_vocabulary(&self) -> ControlledVocabulary {
        let mut mappings = Vec::new();
        for (variant, canonical) in &self.variant_to_canonical {
            mappings.push(VocabularyMapping {
                variant: variant.clone(),
                canonical: canonical.clone(),
            });
        }
        // Sort for deterministic output.
        mappings.sort_by(|a, b| a.variant.cmp(&b.variant));
        ControlledVocabulary { mappings }
    }
}

/// Build a default controlled vocabulary with common software development
/// term mappings.
pub fn default_vocabulary() -> ControlledVocabulary {
    ControlledVocabulary {
        mappings: vec![
            VocabularyMapping {
                variant: "auth".into(),
                canonical: "authentication".into(),
            },
            VocabularyMapping {
                variant: "authn".into(),
                canonical: "authentication".into(),
            },
            VocabularyMapping {
                variant: "authz".into(),
                canonical: "authorization".into(),
            },
            VocabularyMapping {
                variant: "db".into(),
                canonical: "database".into(),
            },
            VocabularyMapping {
                variant: "fe".into(),
                canonical: "frontend".into(),
            },
            VocabularyMapping {
                variant: "be".into(),
                canonical: "backend".into(),
            },
            VocabularyMapping {
                variant: "config".into(),
                canonical: "configuration".into(),
            },
            VocabularyMapping {
                variant: "cfg".into(),
                canonical: "configuration".into(),
            },
            VocabularyMapping {
                variant: "env".into(),
                canonical: "environment".into(),
            },
            VocabularyMapping {
                variant: "dep".into(),
                canonical: "dependency".into(),
            },
            VocabularyMapping {
                variant: "deps".into(),
                canonical: "dependency".into(),
            },
            VocabularyMapping {
                variant: "impl".into(),
                canonical: "implementation".into(),
            },
            VocabularyMapping {
                variant: "repo".into(),
                canonical: "repository".into(),
            },
            VocabularyMapping {
                variant: "pr".into(),
                canonical: "pull_request".into(),
            },
            VocabularyMapping {
                variant: "ci".into(),
                canonical: "continuous_integration".into(),
            },
            VocabularyMapping {
                variant: "cd".into(),
                canonical: "continuous_deployment".into(),
            },
            VocabularyMapping {
                variant: "api".into(),
                canonical: "application_programming_interface".into(),
            },
            VocabularyMapping {
                variant: "jwt".into(),
                canonical: "json_web_token".into(),
            },
            VocabularyMapping {
                variant: "ui".into(),
                canonical: "user_interface".into(),
            },
            VocabularyMapping {
                variant: "ux".into(),
                canonical: "user_experience".into(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization() {
        let vocab = default_vocabulary();
        let index = VocabularyIndex::from_vocabulary(&vocab, 200);

        assert_eq!(index.normalize("auth"), "authentication");
        assert_eq!(index.normalize("authn"), "authentication");
        assert_eq!(index.normalize("unknown_term"), "unknown_term");
    }

    #[test]
    fn deduplication_after_normalization() {
        let vocab = default_vocabulary();
        let index = VocabularyIndex::from_vocabulary(&vocab, 200);

        let subjects = vec!["auth".into(), "authentication".into(), "authn".into()];
        let normalized = index.normalize_subjects(&subjects);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0], "authentication");
    }

    #[test]
    fn query_expansion() {
        let vocab = default_vocabulary();
        let index = VocabularyIndex::from_vocabulary(&vocab, 200);

        let expanded = index.expand_query(&["auth".into()]);
        assert!(expanded.contains(&"authentication".to_string()));
        assert!(expanded.contains(&"auth".to_string()));
        assert!(expanded.contains(&"authn".to_string()));
    }

    #[test]
    fn max_headings_limit() {
        let mut index = VocabularyIndex::empty(2);
        assert!(index.add_mapping("a", "canonical_a"));
        assert!(index.add_mapping("b", "canonical_b"));
        // Third canonical term should be rejected.
        assert!(!index.add_mapping("c", "canonical_c"));
        // But adding a variant of an existing canonical should succeed.
        assert!(index.add_mapping("a2", "canonical_a"));
    }
}
