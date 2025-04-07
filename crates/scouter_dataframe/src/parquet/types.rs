use std::fmt::Display;

pub enum BinnedTableName {
    CustomMetric,
    Psi,
    Spc,
}

impl Display for BinnedTableName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinnedTableName::CustomMetric => write!(f, "binned_custom_metric"),
            BinnedTableName::Psi => write!(f, "binned_psi"),
            BinnedTableName::Spc => write!(f, "binned_spc"),
        }
    }
}
