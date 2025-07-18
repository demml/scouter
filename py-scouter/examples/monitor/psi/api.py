# pylint: disable=invalid-name
from contextlib import asynccontextmanager
from pathlib import Path

import numpy as np
import pandas as pd
import uvicorn
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter import (  # type: ignore
    CommonCrons,
    Drifter,
    Feature,
    Features,
    HTTPConfig,
    PsiAlertConfig,
    PsiDriftConfig,
    ScouterClient,
    ScouterQueue,
    QuantileBinning,
    EqualWidthBinning, Manual, Scott
)
from scouter.alert import SlackDispatchConfig
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.util import FeatureMixin

logger = RustyLogger.get_logger(
    LoggingConfig(log_level=LogLevel.Debug),
)


# Simple response model for our post request.
class Response(BaseModel):
    message: str


class PredictRequest(BaseModel, FeatureMixin):
    feature_1: float
    feature_2: float

    # This helper function is necessary to convert Scouter Python types into the appropriate Rust types.
    # This is what the mixin does under the hood.
    # In practice you can use the mixin directly unless you want more control over the conversion.
    def to_features_example(self) -> Features:
        return Features(
            features=[
                Feature.float("feature_1", self.feature_1),
                Feature.float("feature_2", self.feature_2),
            ]
        )


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 1000
    X_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")
    X = pd.DataFrame(X_train, columns=col_names)

    X["category"] = np.random.choice([11, 12, 22, 31, 14], size=n)

    return X


def create_psi_profile() -> Path:
    """Create a PSI profile

    The following example shows how to:

    1. Instantiate the Drifter class and connect to the Scouter client
    2. Create a fake dataframe
    3. Create a PSI profile using the Drifter class
    4. Register the profile with the Scouter client and set it as active
    (this will tell the server to schedule the profile for alerting)
    5. Save the profile to a json file (we'll use this to load it in the api for demo purposes)
    """
    # Drifter class for creating drift profiles
    drifter = Drifter()

    # Simple client to register drift profiles (scouter client must be running)
    client = ScouterClient()

    # create fake data
    data = generate_data()
    # create psi configuration
    psi_config = PsiDriftConfig(
        space="scouter",
        name="psi_test",
        version="0.0.1",
        alert_config=PsiAlertConfig(
            schedule=CommonCrons.Every6Hours,  # You can also use a custom cron expression
            features_to_monitor=[  # we only want to monitor these features
                "feature_1",
                "feature_2",
            ],
            dispatch_config=SlackDispatchConfig(channel="test")
        ),
        categorical_features=["category"],
        binning_strategy=EqualWidthBinning(method=Scott())
    )

    # create psi profile
    psi_profile = drifter.create_drift_profile(data, psi_config)

    # register profile
    client.register_profile(profile=psi_profile, set_active=True)

    # save profile to json (for example purposes)
    return psi_profile.save_to_json()


if __name__ == "__main__":
    # Create a PSI profile and get its path
    profile_path = create_psi_profile()

    # # Setup the FastAPI app
    # config = HTTPConfig()
    #
    # # Setup api lifespan
    # @asynccontextmanager
    # async def lifespan(fast_app: FastAPI):
    #     logger.info("Starting up FastAPI app")
    #
    #     fast_app.state.queue = ScouterQueue.from_path(
    #         path={"psi": profile_path},
    #         transport_config=config,
    #     )
    #     yield
    #
    #     logger.info("Shutting down FastAPI app")
    #     # Shutdown the queue
    #     fast_app.state.queue.shutdown()
    #     fast_app.state.queue = None
    #
    # app = FastAPI(lifespan=lifespan)
    #
    # @app.post("/predict", response_model=Response)
    # async def predict(request: Request, payload: PredictRequest) -> Response:
    #     request.app.state.queue["psi"].insert(PredictRequest(feature_1=12.2, feature_2=np.nan).to_features())
    #     return Response(message="success")
    #
    # uvicorn.run(app, host="0.0.0.0", port=8888)
