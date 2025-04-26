from pydantic import BaseModel
from scouter import Feature, Features
from scouter.decorator import scouter_features


# Simple response model for our post request.
class Response(BaseModel):
    message: str


@scouter_features
class PredictRequest(BaseModel):
    feature_1: float
    feature_2: float
    feature_2: float

    # This helper function is necessary to convert Scouter Python types into the appropriate Rust types.
    # This is what the decorator does for us.
    # In practice you can just use the decorator
    def to_features_example(self) -> Features:
        return Features(
            features=[
                Feature.float("feature_1", self.feature_1),
                Feature.float("feature_2", self.feature_2),
                Feature.float("feature_3", self.feature_3),
            ]
        )
