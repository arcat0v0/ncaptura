use std::time::Duration;

use super::*;

fn test_video(description: &str) -> gst::Element {
    gst::init().unwrap();
    gst::parse::bin_from_description(description, true)
        .unwrap()
        .upcast()
}

fn inspect_recording(path: &Path) -> serde_json::Value {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_packets",
            "-show_entries",
            "stream=codec_name,codec_type,width,height,nb_read_packets:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("需要安装 ffmpeg 以使用 ffprobe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let duration: f64 = metadata["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(duration.is_finite() && duration > 0.0);
    for stream in metadata["streams"].as_array().unwrap() {
        assert!(
            stream["nb_read_packets"]
                .as_str()
                .unwrap()
                .parse::<u32>()
                .unwrap()
                > 0
        );
    }
    metadata
}

#[tokio::test]
#[ignore = "需要 GStreamer 编码插件和 ffprobe，不使用桌面或真实音视频"]
async fn window_recording_encodes_video_with_and_without_audio() {
    for with_audio in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recording with spaces.mkv");
        let video = test_video(
            "videotestsrc num-buffers=30 ! capsfilter caps=video/x-raw,width=320,height=240,framerate=30/1",
        );
        let audio = with_audio.then(|| {
            gst::ElementFactory::make("audiotestsrc")
                .property("num-buffers", 30i32)
                .build()
                .unwrap()
        });
        let recording = WindowRecordingPipeline::new(video, audio, &path).unwrap();
        recording.run(std::future::pending()).await.unwrap();
        assert_eq!(recording.pipeline.current_state(), gst::State::Null);
        let metadata = inspect_recording(&path);
        let streams = metadata["streams"].as_array().unwrap();
        assert_eq!(streams.len(), if with_audio { 2 } else { 1 });
        assert_eq!(streams[0]["codec_name"], "h264");
        assert_eq!(streams[0]["width"], 320);
        assert_eq!(streams[0]["height"], 240);
        if with_audio {
            assert_eq!(streams[1]["codec_name"], "vorbis");
        }
    }
}

#[tokio::test]
#[ignore = "需要 GStreamer 编码插件和 ffprobe，不使用桌面或真实音视频"]
async fn window_recording_resize_keeps_a_stable_canvas() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("resize.mkv");
    let video = test_video(
        "videotestsrc is-live=true num-buffers=45 ! capsfilter name=input_caps caps=video/x-raw,width=321,height=241,framerate=30/1",
    );
    let input_caps = video
        .clone()
        .downcast::<gst::Bin>()
        .unwrap()
        .by_name("input_caps")
        .unwrap();
    let recording = WindowRecordingPipeline::new(video, None, &path).unwrap();
    let resize = async {
        tokio::time::sleep(Duration::from_millis(350)).await;
        input_caps.set_property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("width", 640i32)
                .field("height", 360i32)
                .field("framerate", gst::Fraction::new(30, 1))
                .build(),
        );
    };
    let (result, ()) = tokio::join!(recording.run(std::future::pending()), resize);
    result.unwrap();
    let metadata = inspect_recording(&path);
    assert_eq!(metadata["streams"][0]["width"], 322);
    assert_eq!(metadata["streams"][0]["height"], 242);
    assert!(
        metadata["format"]["duration"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
            > 1.0
    );
}

#[tokio::test]
#[ignore = "需要 GStreamer 编码插件和 ffprobe，不使用桌面或真实音视频"]
async fn window_recording_stop_finalizes_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stop.mkv");
    let video = test_video(
        "videotestsrc is-live=true ! capsfilter caps=video/x-raw,width=320,height=240,framerate=30/1",
    );
    let recording = WindowRecordingPipeline::new(video, None, &path).unwrap();
    recording
        .run(tokio::time::sleep(Duration::from_millis(400)))
        .await
        .unwrap();
    assert_eq!(recording.pipeline.current_state(), gst::State::Null);
    inspect_recording(&path);
}

#[tokio::test]
#[ignore = "需要 GStreamer 编码插件，不使用桌面或真实音视频"]
async fn window_recording_invalid_output_returns_error_and_stops_pipeline() {
    let directory = tempfile::tempdir().unwrap();
    let video = test_video("videotestsrc num-buffers=5");
    let recording = WindowRecordingPipeline::new(video, None, directory.path()).unwrap();
    assert!(recording.run(std::future::pending()).await.is_err());
    assert_eq!(recording.pipeline.current_state(), gst::State::Null);
}

#[test]
fn window_canvas_accepts_positive_dimensions_and_rounds_up_to_even() {
    gst::init().unwrap();
    for (width, height, expected_width, expected_height) in
        [(320, 240, 320, 240), (321, 241, 322, 242), (1, 1, 2, 2)]
    {
        let caps = gst::Caps::builder("video/x-raw")
            .field("width", width)
            .field("height", height)
            .build();
        let canvas = window_canvas_caps(&caps).unwrap();
        let structure = canvas.structure(0).unwrap();
        assert_eq!(structure.get::<i32>("width").unwrap(), expected_width);
        assert_eq!(structure.get::<i32>("height").unwrap(), expected_height);
    }
}

#[test]
fn window_canvas_rejects_invalid_dimensions() {
    gst::init().unwrap();
    for (width, height) in [(0, 240), (320, 0), (-1, 240), (320, -1), (i32::MAX, 240)] {
        let caps = gst::Caps::builder("video/x-raw")
            .field("width", width)
            .field("height", height)
            .build();
        assert!(window_canvas_caps(&caps).is_err());
    }
    assert!(window_canvas_caps(&gst::Caps::new_empty()).is_err());
}

#[tokio::test]
#[ignore = "需要 GStreamer 编码插件，不使用桌面或真实音视频"]
async fn window_recording_can_stop_before_the_first_frame() {
    let directory = tempfile::tempdir().unwrap();
    let video =
        test_video("videotestsrc is-live=true ! valve drop=true drop-mode=forward-sticky-events");
    let recording =
        WindowRecordingPipeline::new(video, None, &directory.path().join("empty.mkv")).unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(7),
        recording.run(tokio::time::sleep(Duration::from_millis(100))),
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(recording.pipeline.current_state(), gst::State::Null);
}
