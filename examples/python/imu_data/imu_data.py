import pandas as pd
import rerun as rr


def main():
    rr.init("rerun_example_imu_data")
    imu_data = pd.read_csv("./dataset-corridor4_512_16/mav0/imu0/data.csv")

    print(imu_data)
