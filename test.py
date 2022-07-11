import rerun_sdk


print(rerun_sdk.info())
rerun_sdk.log_point("point", 42, 1337)
