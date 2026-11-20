"""Take screenshots of a running Viewer, as a file or as PNG bytes."""

from rerun.experimental import ViewerClient

client = ViewerClient.connect()

# Save a PNG file on the machine that runs the Viewer:
client.save_screenshot("viewer.png")

# Get a PNG as bytes, without writing a file:
png_bytes = client.take_screenshot(max_size=1024)
