package io.rerun.viewer;

import android.os.Bundle;
import android.util.Log;
import android.view.InputDevice;
import android.view.MotionEvent;

import com.google.androidgamesdk.GameActivity;

/**
 * Custom Activity that extends GameActivity to ensure touch events
 * are properly forwarded to the native Rust code.
 *
 * The native android_native_app_glue has a default_motion_filter that
 * does an exact equality check: event->source == SOURCE_TOUCHSCREEN (0x1002).
 * Some devices and emulators report combined source flags (e.g. 0x5002 =
 * TOUCHSCREEN | STYLUS), which the filter silently rejects.
 *
 * We fix this by normalizing the event source to SOURCE_TOUCHSCREEN
 * before it reaches the native code.
 */
public class RerunActivity extends GameActivity {

    private static final String TAG = "RerunActivity";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Log.i(TAG, "RerunActivity created");
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        // Normalize the source to SOURCE_TOUCHSCREEN so the native
        // motion event filter accepts it. Some emulators and stylus-capable
        // devices report combined sources (e.g. 0x5002 = TOUCHSCREEN|STYLUS)
        // which the C-level default_motion_filter rejects via exact match.
        int originalSource = event.getSource();
        if ((originalSource & InputDevice.SOURCE_TOUCHSCREEN) != 0) {
            event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        }
        return super.onTouchEvent(event);
    }
}
