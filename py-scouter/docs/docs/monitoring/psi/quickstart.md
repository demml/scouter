Population Stability Index (PSI) is one of the most common ways to monitor ML models in production. The following sections will walk you throug:

- Setting up a PSI drift profile
- Configuring real-time notifications for model drift detection

### Creating a Drift Profile
To detect model drift, we first need to create a drift profile using your baseline dataset, this is typically done at the time of training your model.
```python
from scouter.alert import PsiAlertConfig, SlackDispatchConfig
from scouter.client import ScouterClient
from scouter.drift import Drifter, PsiDriftConfig
from scouter.types import CommonCrons
from sklearn import datasets

# Prepare data
X, y = datasets.load_wine(return_X_y=True, as_frame=True)

# Drifter class for creating drift profiles
scouter = Drifter()

# Specify the alert configuration
alert_config = PsiAlertConfig(
    features_to_monitor=["malic_acid", "total_phenols", "color_intensity"], # Defaults to all features if left empty
    schedule=CommonCrons.EveryDay, # Run drift detection job once daily
    dispatch_config=SlackDispatchConfig(channel="test_channel"), # Notify my team Slack channel if drift is detected
    # psi_threshold=0.25  # (default) adjust if needed
)

# Create drift config
psi_config = PsiDriftConfig(
    name="wine_model",
    space="wine_model",
    version="0.0.1",
    alert_config=alert_config
)

# Create the drift profile
psi_profile = scouter.create_drift_profile(X, psi_config)

# Register your profile with scouter server
client = ScouterClient()

# set_active must be set to True if you want scouter server to run the drift detection job
# You can always set this later
client.register_profile(profile=psi_profile, set_active=True)
```

!!!note
    Your drift profile is now registered with the `Scouter` server and is ready to be used. To run real-time monitoring, refer to the [Scouter Queues](#) section for more information on how to set up your queues and send data to the Scouter server.