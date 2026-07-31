//! Parameter tree — hierarchical grouping of available parameters for the browser.

use std::collections::BTreeMap;

use crate::fdr_reader::ParameterInfo;

/// Hierarchical tree: Group -> List of Parameters within that group.
#[derive(Debug, Clone)]
pub struct ParameterTree {
    pub groups: BTreeMap<String, Vec<ParameterInfo>>,
}

impl ParameterTree {
    /// Build a tree from a flat list of parameters, grouped by their `group` field.
    pub fn build(params: &[ParameterInfo]) -> Self {
        let mut groups: BTreeMap<String, Vec<ParameterInfo>> = BTreeMap::new();

        for p in params {
            groups.entry(p.group.clone()).or_default().push(p.clone());
        }

        Self { groups }
    }

    /// Look up a specific group by name (case-insensitive).
    pub fn get_group(&self, name: &str) -> Option<&Vec<ParameterInfo>> {
        self.groups.iter().find_map(|(k, v)| {
            if k.eq_ignore_ascii_case(name) {
                Some(v)
            } else {
                None
            }
        })
    }

    /// Filter the tree to only show parameters whose display name contains `query`.
    pub fn filtered<'a>(&'a self, query: &str) -> BTreeMap<String, Vec<&'a ParameterInfo>> {
        let mut result = BTreeMap::new();

        for (group, params) in &self.groups {
            let matches: Vec<_> = params
                .iter()
                .filter(|p| {
                    p.display_name
                        .to_lowercase()
                        .contains(&query.to_lowercase())
                        || p.key.to_lowercase().contains(&query.to_lowercase())
                })
                .collect();

            if !matches.is_empty() {
                result.insert(group.clone(), matches);
            }
        }

        result
    }
}
