from typing import Any

from pydantic import BaseModel
from scouter.queue import KafkaConfig


class Config(BaseModel):
    kafka: Any
    spc_id: str
    psi_id: str
    custom_id: str


config = Config(
    kafka=KafkaConfig(),
    spc_id="spc_id",
    psi_id="psi_id",
    custom_id="custom_id",
)
