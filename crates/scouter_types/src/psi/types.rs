use crate::psi::profile::FeatureBinProportions;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinnedPsiMetric {
    pub date: Vec<NaiveDateTime>,
    pub psi: Vec<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BinnedPsiFeatureMetrics {
    pub features: BTreeMap<String, BinnedPsiMetric>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PsiDriftViz {
    pub feature_metrics: BinnedPsiFeatureMetrics,
    pub bin_proportions: FeatureBinProportions,
}
