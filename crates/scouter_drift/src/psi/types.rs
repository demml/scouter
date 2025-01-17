use scouter_error::DriftError;
use scouter_types::psi::{FeatureBinProportions, PsiFeatureDriftProfile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinProportionPairs {
    pub pairs: Vec<(f64, f64)>,
}

impl FeatureBinProportionPairs {
    pub fn from_observed_bin_proportions(
        observed_bin_proportions: &HashMap<String, f64>,
        profile: &PsiFeatureDriftProfile,
    ) -> Result<Self, DriftError> {
        let pairs: Vec<(f64, f64)> = profile
            .bins
            .iter()
            .map(|bin| {
                let observed_proportion = *observed_bin_proportions
                    .get(&bin.id)
                    .ok_or_else(|| {
                        error!(
                            "Error: Unable to fetch observed bin proportion for {}/{}",
                            profile.id, bin.id
                        );
                        DriftError::Error("Error processing alerts".to_string())
                    })
                    .unwrap();
                (bin.proportion, observed_proportion)
            })
            .collect();

        Ok(Self { pairs })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinMapping {
    pub features: HashMap<String, FeatureBinProportionPairs>,
}

impl FeatureBinMapping {
    pub fn from_observed_bin_proportions(
        observed_bin_proportions: &FeatureBinProportions,
        profiles_to_monitor: &[PsiFeatureDriftProfile],
    ) -> Result<Self, DriftError> {
        let features: HashMap<String, FeatureBinProportionPairs> = profiles_to_monitor
            .iter()
            .map(|profile| {
                let proportion_pairs = FeatureBinProportionPairs::from_observed_bin_proportions(
                    observed_bin_proportions.features.get(&profile.id).unwrap(),
                    profile,
                )
                .unwrap();
                (profile.id.clone(), proportion_pairs)
            })
            .collect();

        Ok(Self { features })
    }
}
