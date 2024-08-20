use std::path::Path;

use re_video::VideoData;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

#[test]
fn parse_mp4() {
    let file = std::fs::read(Path::new(DATA_DIR).join("bbb_video_av1_frag.mp4")).unwrap();
    let video = VideoData::load_mp4(&file).unwrap();

    println!("{video:#?}");

    panic!();
}
