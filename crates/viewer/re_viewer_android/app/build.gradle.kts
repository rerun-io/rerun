import java.io.ByteArrayOutputStream

plugins {
    id("com.android.application")
}

android {
    namespace = "io.rerun.viewer"
    compileSdk = 35

    defaultConfig {
        applicationId = "io.rerun.viewer"
        minSdk = 28 // Android 9 (Pie) -- minimum for good Vulkan support
        targetSdk = 35
        versionCode = 1
        versionName = "0.25.0"

        ndk {
            // Target ARM64 devices and x86_64 emulators
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    // The Rust native library is pre-built and placed in jniLibs by cargo-ndk.
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

configurations.all {
    resolutionStrategy {
        // Force a single kotlin-stdlib version to avoid duplicate class conflicts
        // between kotlin-stdlib and kotlin-stdlib-jdk8
        force("org.jetbrains.kotlin:kotlin-stdlib:1.8.22")
        force("org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.8.22")
        force("org.jetbrains.kotlin:kotlin-stdlib-jdk7:1.8.22")
    }
}

dependencies {
    // GameActivity is required for the android-game-activity winit backend.
    // It extends AppCompatActivity, so appcompat must also be included.
    // Must match the version bundled by the Rust `android-activity` crate (0.6.0 bundles 2.0.2)
    implementation("androidx.games:games-activity:2.0.2")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.core:core:1.15.0")
}

/**
 * Task to build the Rust native library using cargo-ndk.
 *
 * This compiles re_viewer as a cdylib for the configured Android ABIs
 * and places the resulting .so files into the jniLibs directory.
 *
 * Prerequisites:
 *   - cargo-ndk: `cargo install cargo-ndk`
 *   - Android NDK: installed via Android Studio SDK Manager
 *   - Rust targets: `rustup target add aarch64-linux-android x86_64-linux-android`
 */
tasks.register<Exec>("buildRustLib") {
    description = "Build the Rerun Viewer Rust library for Android using cargo-ndk"

    val rerunRoot = file("${project.rootDir}/../../..") // Navigate up to the rerun workspace root
    workingDir = rerunRoot

    val jniLibsDir = file("src/main/jniLibs")

    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        "-t", "x86_64",
        "-o", jniLibsDir.absolutePath,
        "build",
        "--release",
        "-p", "re_viewer",
        "--lib",
        "--features", "android-game-activity"
    )
}

// Wire the Rust build into the Android build pipeline.
// Run `./gradlew buildRustLib` manually before building the APK,
// or uncomment the line below to build automatically (slower builds).
// tasks.named("preBuild") { dependsOn("buildRustLib") }
