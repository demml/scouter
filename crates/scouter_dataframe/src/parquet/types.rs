use std::fmt::Display;

pub enum BinnedTableName {
    CustomMetric,
    Psi,
    Spc,
    GenAITask,
    GenAIWorkflow,
    GenAIEval,
}

impl Display for BinnedTableName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinnedTableName::CustomMetric => write!(f, "binned_custom_metric"),
            BinnedTableName::Psi => write!(f, "binned_psi"),
            BinnedTableName::Spc => write!(f, "binned_spc"),
            BinnedTableName::GenAITask => write!(f, "binned_genai_task"),
            BinnedTableName::GenAIWorkflow => write!(f, "binned_genai_workflow"),
            BinnedTableName::GenAIEval => write!(f, "binned_genai_event"),
        }
    }
}
