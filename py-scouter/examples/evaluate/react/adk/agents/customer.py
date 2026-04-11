"""
ADK customer simulation agent.

No callback — this agent is not evaluated. It simulates a home cook driving
the conversation. Each turn it receives the original goal and the recipe
agent's latest response, then either asks a follow-up or outputs SATISFIED.
"""

from google.adk.agents import Agent

from ..setup import config

customer_agent = Agent(
    model=config.customer_prompt.model,
    name="customer_agent",
    description="Simulates a home cook exploring recipe options.",
    instruction=config.customer_prompt.message.text,
)
