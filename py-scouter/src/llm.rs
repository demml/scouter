use potato_head::{
    AudioUrl, BinaryContent, ChatResponse, DocumentUrl, EventDetails, ImageUrl, Message,
    ModelSettings, Prompt, Provider, PyAgent, PyTask, PyWorkflow, Score, Task, TaskEvent,
    WorkflowResult,
};
use pyo3::prelude::*;
#[pymodule]
pub fn llm(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Provider>()?;
    m.add_class::<PyAgent>()?;
    m.add_class::<PyWorkflow>()?;
    m.add_class::<Task>()?;
    m.add_class::<PyTask>()?;
    m.add_class::<Prompt>()?;
    m.add_class::<Message>()?;
    m.add_class::<ModelSettings>()?;
    m.add_class::<Score>()?;
    m.add_class::<AudioUrl>()?;
    m.add_class::<BinaryContent>()?;
    m.add_class::<DocumentUrl>()?;
    m.add_class::<ImageUrl>()?;
    m.add_class::<EventDetails>()?;
    m.add_class::<ChatResponse>()?;
    m.add_class::<TaskEvent>()?;
    m.add_class::<WorkflowResult>()?;
    Ok(())
}
