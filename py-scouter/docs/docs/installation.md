You can install Scouter from any python package manager

=== "uv"

    ```console
    uv add scouter-ml
    ```

=== "pip"

    ```console
    pip install scouter-ml
    ```

## Importing

To use Scouter, simply import it in your Python script:

```python
import scouter

from scouter import Drifter
```

## Connect to Scouter Server

The Scouter python client is a client-side library that is meant to be used in conjunction with the Scouter server. To use the Scouter client, you need to set the `SCOUTER_SERVER_URI` environment variable to point to your Scouter server. This can be done in your terminal or in your Python script.

```bash
export SCOUTER_SERVER_URI=your_SCOUTER_SERVER_URI
```

For more information on how to setup the Scouter server, refer to the Server documentation.