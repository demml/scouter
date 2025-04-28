Statistical Process Control (SPC) is a powerful tool for monitoring and controlling processes. In this guide, we will walk you through the steps to set up SPC for your model using Scouter.

### Creating a Drift Profile
To detect model drift, we first need to create a drift profile using your training data, but before doing that we will define a custom SPC alert rule.


```python
from scouter.alert import SlackDispatchConfig, SpcAlertConfig, SpcAlertRule
from scouter.client import ScouterClient
from scouter.drift import Drifter, SpcDriftConfig
from scouter.types import CommonCrons
from sklearn import datasets

if __name__ == "__main__":
    # Prepare data
    X, y = datasets.load_wine(return_X_y=True, as_frame=True)

    # Drifter class to create drift profiles
    scouter = Drifter()

    # Specify the alert configuration
    alert_config = SpcAlertConfig(
        features_to_monitor=["malic_acid", "total_phenols", "color_intensity"], # Defaults to all features if left empty
        schedule=CommonCrons.EveryDay, # Run drift detection job once daily
        dispatch_config=SlackDispatchConfig(channel="test_channel"), # Notify my team Slack channel if drift is detected
        rule=SpcAlertRule(rule="16 32 4 8 2 4 1 1"), # See the spc theory doc for additional info
    )

    # Set up SPC drift config with a custom sample size
    spc_config = SpcDriftConfig(name="wine_model", space="wine_model", version="0.0.1", alert_config=alert_config, sample_size=1000)

    # Create the drift profile
    spc_profile = scouter.create_drift_profile(X, spc_config)

    # Register your profile with scouter server
    client = ScouterClient()
    # set_active must be set to True if you want scouter server to run the drift detection job
    client.register_profile(profile=spc_profile, set_active=True)
```

!!!note
    Your drift profile is not registered with the `Scouter` server and is ready to be used. To run real-time monitoring, refer to the [Scouter Queues](#) section for more information on how to set up your queues and send data to the Scouter server in real-time.
