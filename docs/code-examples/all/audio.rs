//! Log a .wav audio file to Rerun

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let rec = rerun::RecordingStreamBuilder::new("rerun_example_audio").spawn()?;
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_audio").save("../audio2.rrd")?;

    let audio = rerun::Audio::from_wav_bytes(include_bytes!("../../rerun-music.wav"))?;
    rec.log("music", &audio)?;

    let audio = rerun::Audio::from_wav_bytes(include_bytes!("../../icq.wav"))?;

    rec.log("icq", &audio)?;

    // Log it multiple times so playback is interesting:
    std::thread::sleep(std::time::Duration::from_secs(1));
    rec.log("icq", &audio)?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    rec.log("icq", &audio)?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    rec.log("icq", &audio)?;

    Ok(())
}
