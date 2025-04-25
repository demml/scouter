from pydantic import BaseModel
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
    # def to_features(self) -> Features:
    #    return Features(
    #        features=[
    #            Feature.float("feature_1", self.feature_1),
    #            Feature.float("feature_2", self.feature_2),
    #            Feature.float("feature_3", self.feature_3),
    #        ]
    #    )


#

if __name__ == "__main__":
    # Example usage
    request = PredictRequest(feature_1=1.0, feature_2=2.0, feature_3=3.0)
