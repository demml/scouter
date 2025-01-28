from pathlib import Path

import numpy as np
import pandas as pd
from scouter.client import ScouterClient
from scouter.drift import Drifter, SpcDriftConfig, PsiDriftConfig
from scouter.types import CommonCrons
from scouter.alert import SpcAlertConfig



def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000

    X_train = np.random.normal(-4, 2.0, size=(n, 4))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)

    return X


if __name__ == "__main__":

    # Drfter class for creating drift profiles
    scouter = Drifter()
    
    # Simple client to register drift profiles
    client = ScouterClient()

    # create fake data
    data = generate_data()
    
    # create config for population stability index
    config = PsiDriftConfig(
        name="test",
        repository="test",
        version="0.0.1",
    )

    profile = scouter.create_drift_profile(data, config)
    client.register_profile(profile)

    profile.save_to_json(path=Path("drift_profile.json"))
