//! Unified coupling command — groups coupling, coupling-clusters, and hotspots views.

use crate::commands::analyze::coupling::CouplingReport;
use crate::commands::analyze::coupling_clusters::CouplingClustersReport;
use crate::commands::analyze::hotspots::HotspotsReport;
use normalize_output::OutputFormatter;
use serde::Serialize;

/// Coupling analysis output — temporal co-change pairs, clusters, or churn hotspots.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "view")]
pub enum CouplingOutput {
    /// File pairs that change together
    #[serde(rename = "pairs")]
    Pairs(CouplingReport),
    /// File groups (connected components of co-change)
    #[serde(rename = "clusters")]
    Clusters(CouplingClustersReport),
    /// Churn × complexity hotspots
    #[serde(rename = "hotspots")]
    Hotspots(HotspotsReport),
}

impl OutputFormatter for CouplingOutput {
    fn format_text(&self) -> String {
        match self {
            CouplingOutput::Pairs(r) => r.format_text(),
            CouplingOutput::Clusters(r) => r.format_text(),
            CouplingOutput::Hotspots(r) => r.format_text(),
        }
    }

    fn format_pretty(&self) -> String {
        match self {
            CouplingOutput::Pairs(r) => r.format_pretty(),
            CouplingOutput::Clusters(r) => r.format_pretty(),
            CouplingOutput::Hotspots(r) => r.format_pretty(),
        }
    }
}
