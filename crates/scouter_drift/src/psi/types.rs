use crate::error::DriftError;
use scouter_types::psi::{FeatureDistributions, PsiFeatureDriftProfile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinProportionPairs {
    pub bins: Vec<String>,
    pub pairs: Vec<(f64, f64)>,
}

impl FeatureBinProportionPairs {
    pub fn from_observed_bin_proportions( 
        observed_bin_proportions: &BTreeMap<usize, f64>,
        profile: &PsiFeatureDriftProfile,
    ) -> Result<Self, DriftError> {
        let (bins, pairs): (Vec<String>, Vec<(f64, f64)>) = profile
            .bins
            .iter()
            .map(|bin| {
                let observed_proportion = *observed_bin_proportions.get(&bin.id).unwrap_or(&0.0); // It's possible that there is no data for a bin, which would mean 0
                (bin.id.to_string(), (bin.proportion, observed_proportion))
            })
            .unzip();

        Ok(Self { bins, pairs })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinMapping {
    pub features: HashMap<String, FeatureBinProportionPairs>,
}

impl FeatureBinMapping {
    pub fn from_observed_bin_proportions(
        observed_bin_proportions: &FeatureDistributions,
        profiles_to_monitor: &[PsiFeatureDriftProfile],
    ) -> Result<Self, DriftError> {
        let features: HashMap<String, FeatureBinProportionPairs> = profiles_to_monitor
            .iter()
            .map(|profile| {
                let proportion_pairs = FeatureBinProportionPairs::from_observed_bin_proportions(
                    &observed_bin_proportions
                        .distributions
                        .get(&profile.id)
                        .unwrap()
                        .bins,
                    profile,
                )
                .unwrap();
                (profile.id.clone(), proportion_pairs)
            })
            .collect();

        Ok(Self { features })
    }
}
