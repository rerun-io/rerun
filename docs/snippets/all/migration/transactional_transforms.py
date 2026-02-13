import rerun as rr

rr.log("simple", rr.Transform3D(translation=[1.0, 2.0, 3.0]))

# Note that we explicitly only set the scale here:
# Previously, this would have meant that we keep the translation.
# However, in 0.27 the Viewer will no longer show apply the previous translation regardless.
rr.log("simple", rr.Transform3D.from_fields(scale=2))
