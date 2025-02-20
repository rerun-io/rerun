import pandas as pd
import pathlib

import rerun as rr
from rerun import blueprint as rrb

cwd = pathlib.Path(__file__).parent.resolve()


def _setup_rerun() -> None:
    rr.init("rerun_example_imu_data", spawn=True)

    rr.send_blueprint(
        rrb.Horizontal(
            rrb.Vertical(
                rrb.TimeSeriesView(
                    origin="/gyroscope",
                    name="Gyroscope",
                    overrides={
                        "/gyroscope": [
                            rr.components.NameBatch(["x", "y", "z"]),
                            rr.components.ColorBatch([(231, 76, 60), (39, 174, 96), (52, 120, 219)]),
                        ]
                    },
                ),
                rrb.TimeSeriesView(
                    origin="/accelerometer",
                    name="Accelerometer",
                    overrides={
                        "/accelerometer": [
                            rr.components.NameBatch(["x", "y", "z"]),
                            rr.components.ColorBatch([(231, 76, 60), (39, 174, 96), (52, 120, 219)]),
                        ]
                    },
                ),
            )
        )
    )


def main() -> None:
    imu_data = pd.read_csv(cwd / "dataset-corridor4_512_16/mav0/imu0/data.csv")

    print(imu_data)

    times = rr.TimeNanosColumn("timestamp", imu_data["#timestamp [ns]"])

    gyro = imu_data[["w_RS_S_x [rad s^-1]", "w_RS_S_y [rad s^-1]", "w_RS_S_z [rad s^-1]"]]
    rr.send_columns("/gyroscope", indexes=[times], columns=rr.Scalar.columns(scalar=gyro))

    accel = imu_data[["a_RS_S_x [m s^-2]", "a_RS_S_y [m s^-2]", "a_RS_S_z [m s^-2]"]]
    rr.send_columns("/accelerometer", indexes=[times], columns=rr.Scalar.columns(scalar=accel))


if __name__ == "__main__":
    _setup_rerun()
    main()
