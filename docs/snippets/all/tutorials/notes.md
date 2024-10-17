
Blockers to landing PR
- APIs marked as experimental
- Publish forked version of `mp4` library (`re_mp4`?)
- On native we just create an empty texture
- If user only logs a video file, they get the truncated timeline issue

Blockers to stabilizing video support:
- Documented feature matrix: platforms/browsers x containers x codecs
- Error message on unsupported visualizers
- Standardize the "no video support" empty texture
- Implement some minimal form of video-frame-reference components that expand the timeline
- Mapping of frames to ticks on a sequence timeline

Native:
- General problem the will surface is licensing
- Exa: there's an "open" H264 library but still requires licensing
- Note this might inform what we want to support. Even if web supports it, if native licensing is a problem, we might not want to support it yet.
- Using ffmpeg from a system-library installation seems plausible (also need to be careful about licensing)
- Probably need some UI/Design work to handle missing support.
    - Both missing codecs, missing ffmpeg lib, etc.
- Is it possible to do this with a native web-view?

Video frame references:
- HF current solution:
    - `Struct<uri: String, pts: Timestamp>`
- GOALS:
    - End up with a per-frame place-holder component that shows up in the timeline.
    - Does this place-holder specify PTS or FrameNumber? Maybe either?
- Options:
    - Make user do this themselves
    - Rust code that processes the mp4 and does this for the user
    - Create a timeline from the video directly
- Open question:
- What is the reference referencing?
    - An implicit component on the same entity? LatestVideo
    - An entity + row?
    - A "blob asset"?


