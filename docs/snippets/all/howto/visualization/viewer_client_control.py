"""Open a recording in a running Viewer and move its time cursor."""

from rerun.experimental import ViewerClient

client = ViewerClient.connect()  # rerun+http://127.0.0.1:9876/proxy by default
client.open("recording.rrd")
client.set_time("frame", sequence=120, play_state="paused")
client.set_time(play_state="playing")
# The path is on the machine that runs the Viewer:
client.save_screenshot("viewer.png")
client.close("current")
