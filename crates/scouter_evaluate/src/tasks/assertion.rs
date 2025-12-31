use crate::tasks::evaluator::AssertionEvaluator;
use crate::tasks::traits::AssertionAccessor;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use scouter_types::genai::{AssertionResult, AssertionTask, EvaluationContext};
use serde_json::Value;

impl AssertionAccessor for AssertionTask {
    fn field_path(&self) -> &Option<String> {
        &self.field_path
    }

    fn id(&self) -> &String {
        &self.id
    }

    fn operator(&self) -> &scouter_types::genai::ComparisonOperator {
        &self.operator
    }

    fn expected_value(&self) -> &Value {
        &self.expected_value
    }

    fn depends_on(&self) -> &Vec<String> {
        &self.depends_on
    }
}

impl EvaluationTask for AssertionTask {
    fn execute(&self, context: &EvaluationContext) -> Result<AssertionResult, EvaluationError> {
        let context = context.build_task_context(self.depends_on())?;
        let result = AssertionEvaluator::evaluate_assertion(&context, self)?;

        Ok(result)
    }
}
