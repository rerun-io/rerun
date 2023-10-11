use re_viewer_context::{SystemCommand, SystemCommandSender};

pub(super) fn python_quick_start(
    command_sender: &re_viewer_context::CommandSender,
) -> re_sdk::RecordingStreamResult<()> {
    let (rec, storage) = re_sdk::RecordingStreamBuilder::new("Python Getting Started").memory()?;

    rec.log(
        "markdown",
        &re_sdk::TextDocument::new(
            r#"
## Python Quick Start

### Installing the Rerun SDK

The Rerun SDK is available on [PyPI](https://pypi.org/) under the
[`rerun-sdk`](https://pypi.org/project/rerun-sdk/) name. It can be installed like any other
Python package:

```sh
pip3 install rerun-sdk
```

### Try out the viewer

The Rerun SDK comes with a demo that can be used to try the viewer. You can send a demo recording
to this viewer using the following command:

```sh
python3 -m rerun_sdk.demo --connect
```

This will open a new recording that looks like this:

![Demo recording](https://static.rerun.io/quickstart2_simple_cube/632a8f1c79f70a2355fad294fe085291fcf3a8ae/768w.png)


### Logging your own data

Instead of a pre-packaged demo, you can log your own data. Copy and paste the following snippet in a new
Python file and execute it to create a new recording in this viewer:

```python
import rerun as rr
import numpy as np

# Initialize the SDK and give our recording a unique name
rr.init("my_own_data")

# Connect to a local viewer using the default port
rr.connect()


# Create some data
SIZE = 10

pos_grid = np.meshgrid(*[np.linspace(-10, 10, SIZE)]*3)
positions = np.vstack([d.reshape(-1) for d in pos_grid]).T

col_grid = np.meshgrid(*[np.linspace(0, 255, SIZE)]*3)
colors = np.vstack([c.reshape(-1) for c in col_grid]).astype(np.uint8).T

# Log the data
rr.log(
    # name under which this entity is logged (known as "entity path")
    "my_points",
    # log data as a 3D point cloud archetype
    rr.Points3D(positions, colors=colors, radii=0.5)
)
```

### How does it work?

TBC
"#
                .trim(),
        )
        .with_media_type(re_sdk::MediaType::markdown()),
    )?;

    let log_msgs = storage.take();
    let store_id = rec.store_info().map(|info| info.store_id.clone());
    command_sender.send_system(SystemCommand::LoadLogMessage(log_msgs));
    if let Some(store_id) = store_id {
        command_sender.send_system(SystemCommand::SetRecordingId(store_id));
    }

    Ok(())
}
