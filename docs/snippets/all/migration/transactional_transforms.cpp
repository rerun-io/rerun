// Log a translation transform.
rec.log("simple", rerun::Transform3D::from_translation({1.0f, 2.0f, 3.0f}));

// Note that we explicitly only set the scale here:
// Previously, this would have meant that we keep the translation.
// However, in 0.27 the Viewer will no longer apply the previous translation regardless.
rec.log("simple", rerun::Transform3D::update_fields().with_scale(2.0f));
