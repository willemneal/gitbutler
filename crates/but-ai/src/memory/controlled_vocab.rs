//! Controlled vocabulary management.
//!
//! Without a controlled vocabulary, the same concept gets classified under
//! "authentication", "auth", "login", "sign-in", and "access control".
//! Searches for any one term miss the others.
//!
//! The controlled vocabulary maps variant terms to canonical terms,
//! ensuring that classification and retrieval use consistent language.

use std::collections::HashMap;

/// A live vocabulary index for term normalization and query expansion.
///
/// Provides fast lookup from variant -> canonical and reverse lookup
/// from canonical -> all variants.
#[derive(Debug, Clone)]
pub struct VocabularyIndex {
    /// variant (lowercased) -> canonical (lowercased).
    variant_to_canonical: HashMap<String, String>,
    /// canonical (lowercased) -> list of variants.
    canonical_to_variants: HashMap<String, Vec<String>>,
}

impl VocabularyIndex {
    /// Create a new empty vocabulary index.
    pub fn new() -> Self {
        Self {
            variant_to_canonical: HashMap::new(),
            canonical_to_variants: HashMap::new(),
        }
    }

    /// Create a vocabulary index with the default software development mappings.
    pub fn with_defaults() -> Self {
        let mut index = Self::new();
        for (variant, canonical) in default_mappings() {
            index.add_mapping(variant, canonical);
        }
        index
    }

    /// Add a synonym mapping (variant -> canonical term).
    pub fn add_mapping(&mut self, variant: &str, canonical: &str) {
        let variant_lower = variant.to_lowercase();
        let canonical_lower = canonical.to_lowercase();

        self.variant_to_canonical
            .insert(variant_lower.clone(), canonical_lower.clone());
        self.canonical_to_variants
            .entry(canonical_lower)
            .or_default()
            .push(variant_lower);
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

    /// Normalize a term to its canonical form.
    /// Returns the original term (lowercased) if unknown.
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

    /// Get all variants for a canonical term.
    pub fn get_variants(&self, canonical: &str) -> &[String] {
        self.canonical_to_variants
            .get(&canonical.to_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Expand a query: given search terms, add all known variants
    /// and canonical forms to broaden the search.
    pub fn expand_query(&self, terms: &[String]) -> Vec<String> {
        let mut expanded = std::collections::HashSet::new();

        for term in terms {
            let lower = term.to_lowercase();
            expanded.insert(lower.clone());

            // If it's a variant, add the canonical form and all siblings.
            if let Some(canonical) = self.variant_to_canonical.get(&lower) {
                expanded.insert(canonical.clone());
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

    /// Number of canonical terms.
    pub fn canonical_count(&self) -> usize {
        self.canonical_to_variants.len()
    }
}

impl Default for VocabularyIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Default software development term mappings.
fn default_mappings() -> Vec<(&'static str, &'static str)> {
    vec![
        ("auth", "authentication"),
        ("authn", "authentication"),
        ("authz", "authorization"),
        ("db", "database"),
        ("fe", "frontend"),
        ("be", "backend"),
        ("config", "configuration"),
        ("cfg", "configuration"),
        ("env", "environment"),
        ("dep", "dependency"),
        ("deps", "dependency"),
        ("impl", "implementation"),
        ("repo", "repository"),
        ("pr", "pull_request"),
        ("ci", "continuous_integration"),
        ("cd", "continuous_deployment"),
        ("api", "application_programming_interface"),
        ("jwt", "json_web_token"),
        ("ui", "user_interface"),
        ("ux", "user_experience"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization() {
        let index = VocabularyIndex::with_defaults();
        assert_eq!(index.normalize("auth"), "authentication");
        assert_eq!(index.normalize("authn"), "authentication");
        assert_eq!(index.normalize("unknown_term"), "unknown_term");
    }

    #[test]
    fn deduplication_after_normalization() {
        let index = VocabularyIndex::with_defaults();
        let subjects = vec!["auth".into(), "authentication".into(), "authn".into()];
        let normalized = index.normalize_subjects(&subjects);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0], "authentication");
    }

    #[test]
    fn query_expansion() {
        let index = VocabularyIndex::with_defaults();
        let expanded = index.expand_query(&["auth".into()]);
        assert!(expanded.contains(&"authentication".to_string()));
        assert!(expanded.contains(&"auth".to_string()));
        assert!(expanded.contains(&"authn".to_string()));
    }

    #[test]
    fn remove_canonical() {
        let mut index = VocabularyIndex::with_defaults();
        index.remove_canonical("authentication");
        assert_eq!(index.normalize("auth"), "auth"); // No longer mapped.
    }
}
