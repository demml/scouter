from typing import Optional, Union

from scouter.utils.logger import ScouterLogger

from .._scouter import CommonCron, DriftType  # pylint: disable=no-name-in-module
from .custom import CustomDrifter
from .standard_drifter import StandardDrifter

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore

DrifterTypes = Union[CustomDrifter, StandardDrifter]


class Drifter:
    @staticmethod
    def build(drift_type: DriftType):
        if drift_type == DriftType.Custom:
            return CustomDrifter()
        return StandardDrifter(drift_type)
