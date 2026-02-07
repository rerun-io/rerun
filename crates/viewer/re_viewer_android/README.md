# Rerun Viewer for Android

This directory contains the Android project that packages the Rerun Viewer as an Android APK.

The Rerun Viewer is written in Rust and rendered using wgpu (Vulkan on Android). This
Gradle project wraps the compiled native library into an Android application using
the GameActivity backend.

## Prerequisites

1. **Android SDK & NDK**: Install via [Android Studio](https://developer.android.com/studio)
   SDK Manager. You need:
   - Android SDK Platform 35
   - Android NDK (latest stable)
   - CMake (optional, not used directly)

2. **Rust Android targets**:
   ```sh
   rustup target add aarch64-linux-android x86_64-linux-android
   ```

3. **cargo-ndk**: Install the cargo-ndk tool for building Rust libraries for Android:
   ```sh
   cargo install cargo-ndk
   ```

4. **Set environment variables**:
   ```sh
   export ANDROID_HOME="$HOME/Library/Android/sdk"  # macOS
   export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/<version>"
   ```

## Building

### Step 1: Build the Rust native library

From the **rerun workspace root**:

```sh
pixi run build-android
```

Or manually:

```sh
cargo ndk -t arm64-v8a -t x86_64 \
  -o crates/viewer/re_viewer_android/app/src/main/jniLibs \
  build --release -p re_viewer --lib --features android-game-activity
```

This places `libre_viewer.so` into the `jniLibs` directory for each ABI.

### Step 2: Build the APK

From this directory:

```sh
./gradlew assembleDebug
```

The APK will be at `app/build/outputs/apk/debug/app-debug.apk`.

### Step 3: Install on device/emulator

```sh
adb install app/build/outputs/apk/debug/app-debug.apk
```

## Device Requirements

- Android 9 (API 28) or higher
- Vulkan 1.1 support (most devices from 2018+)
- ARM64 or x86_64 architecture

## Architecture

The Android app works as follows:

1. `RerunActivity` (extends `GameActivity`) serves as the app entry point. It
   normalizes touch event sources so that the native motion event filter accepts
   input from all pointer devices (touchscreens, styluses, emulators).
2. `GameActivity` loads `libre_viewer.so` (the compiled Rust library)
3. The Rust `android_main()` function is called with the `AndroidApp` handle
4. `android_main()` configures eframe/egui with the Vulkan/wgpu backend and launches
   the standard Rerun Viewer `App`

The viewer shares the same codebase as the desktop viewer, with platform-specific
guards (`#[cfg(target_os = "android")]`) for Android-specific behavior.
