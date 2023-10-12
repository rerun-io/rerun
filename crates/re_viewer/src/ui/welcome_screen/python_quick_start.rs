use re_data_store::StoreDb;
use re_log_types::{
    ApplicationId, DataRow, EntityPath, RowId, StoreId, StoreInfo, StoreKind, StoreSource, Time,
    TimePoint,
};
use re_viewer_context::{SystemCommand, SystemCommandSender};

pub(super) fn python_quick_start(
    command_sender: &re_viewer_context::CommandSender,
) -> anyhow::Result<()> {
    let text_doc =
        re_types::archetypes::TextDocument::new(
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
        .with_media_type(re_types::components::MediaType::markdown());

    let row = DataRow::from_archetype(
        RowId::random(),
        TimePoint::timeless(),
        EntityPath::from("quick_start"),
        &text_doc,
    )?;

    let store_info = StoreInfo {
        application_id: ApplicationId::from("Python Quick Start"),
        store_id: StoreId::random(StoreKind::Recording),
        is_official_example: true,
        started: Time::now(),
        store_source: StoreSource::InAppGuides,
        store_kind: StoreKind::Recording,
    };

    let store_db = StoreDb::from_info_and_rows(store_info, [row])?;

    command_sender.send_system(SystemCommand::LoadStoreDb(store_db));

    Ok(())
}
