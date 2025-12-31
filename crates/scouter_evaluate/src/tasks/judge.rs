use crate::error::EvaluationError;
use crate::tasks::evaluator::AssertionEvaluator;
use crate::tasks::traits::AssertionAccessor;
use crate::tasks::traits::EvaluationTask;
use scouter_types::genai::AssertionResult;
use scouter_types::genai::EvaluationContext;
use scouter_types::genai::LLMJudgeTask;

impl AssertionAccessor for LLMJudgeTask {
    fn field_path(&self) -> &Option<String> {
        &self.field_path
    }

    fn id(&self) -> &String {
        &self.id
    }

    fn operator(&self) -> &scouter_types::genai::ComparisonOperator {
        &self.operator
    }

    fn expected_value(&self) -> &scouter_types::genai::AssertionValue {
        &self.expected_value
    }
    fn depends_on(&self) -> &Vec<String> {
        &self.depends_on
    }
}

impl EvaluationTask for LLMJudgeTask {
    fn execute(&self, context: &EvaluationContext) -> Result<AssertionResult, EvaluationError> {
        let context = context.build_task_context(self.depends_on())?;
        let result = AssertionEvaluator::evaluate_assertion(&context, self)?;

        Ok(result)
    }
}
