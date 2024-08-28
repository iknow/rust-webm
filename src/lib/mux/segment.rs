use ffi::mux::{TrackNum, RESULT_OK};

use crate::ffi;

use std::ptr::NonNull;

use super::{AudioCodecId, AudioTrackNum, Error, MkvWriter, VideoCodecId, VideoTrackNum};

pub struct Segment<W> {
    ffi: Option<ffi::mux::SegmentNonNullPtr>,
    writer: Option<W>,
}

unsafe impl<W: Send> Send for Segment<W> {}

impl<W> Segment<W> {
    /// Note: the supplied writer must have a lifetime larger than the segment.
    pub fn new(dest: W) -> Result<Self, Error>
    where
        W: MkvWriter,
    {
        let ffi = unsafe { ffi::mux::new_segment() };
        let ffi = NonNull::new(ffi).ok_or(Error::Unknown)?;
        let result = unsafe { ffi::mux::initialize_segment(ffi.as_ptr(), dest.mkv_writer()) };
        match result {
            RESULT_OK => Ok(Segment {
                ffi: Some(ffi),
                writer: Some(dest),
            }),
            _ => {
                unsafe {
                    ffi::mux::delete_segment(ffi.as_ptr());
                }
                Err(Error::Unknown)
            }
        }
    }

    pub fn set_app_name(&mut self, name: &str) {
        let name = std::ffi::CString::new(name).unwrap();
        unsafe {
            ffi::mux::mux_set_writing_app(self.segment_ptr(), name.as_ptr());
        }
    }

    pub fn add_video_track(
        &mut self,
        width: u32,
        height: u32,
        track_num: Option<i32>,
        codec: VideoCodecId,
    ) -> Result<VideoTrackNum, Error> {
        let mut track_num_out: TrackNum = 0;
        let result = unsafe {
            ffi::mux::segment_add_video_track(
                self.segment_ptr(),
                width as i32,
                height as i32,
                track_num.unwrap_or(0),
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            RESULT_OK => {
                assert_ne!(track_num_out, 0);
                Ok(VideoTrackNum(track_num_out))
            }
            _ => Err(Error::Unknown),
        }
    }

    pub fn add_audio_track(
        &mut self,
        sample_rate: i32,
        channels: i32,
        track_num: Option<i32>,
        codec: AudioCodecId,
    ) -> Result<AudioTrackNum, Error> {
        let mut track_num_out: TrackNum = 0;
        let result = unsafe {
            ffi::mux::segment_add_audio_track(
                self.segment_ptr(),
                sample_rate,
                channels,
                track_num.unwrap_or(0),
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            RESULT_OK => {
                assert_ne!(track_num_out, 0);
                Ok(AudioTrackNum(track_num_out))
            }
            _ => Err(Error::Unknown),
        }
    }

    pub fn add_frame(
        &mut self,
        track_num: TrackNum,
        data: &[u8],
        timestamp_ns: u64,
        keyframe: bool,
    ) -> Result<(), Error> {
        let result = unsafe {
            ffi::mux::segment_add_frame(
                self.segment_ptr(),
                track_num,
                data.as_ptr(),
                data.len(),
                timestamp_ns,
                keyframe,
            )
        };

        match result {
            RESULT_OK => Ok(()),
            _ => Err(Error::Unknown),
        }
    }

    pub fn set_codec_private(&mut self, track_number: TrackNum, data: &[u8]) -> Result<(), Error> {
        let result = unsafe {
            ffi::mux::segment_set_codec_private(
                self.segment_ptr(),
                track_number,
                data.as_ptr(),
                data.len().try_into().unwrap(),
            )
        };

        match result {
            RESULT_OK => Ok(()),
            _ => Err(Error::Unknown),
        }
    }

    pub fn set_color(
        &mut self,
        track: VideoTrackNum,
        bit_depth: u8,
        subsampling: (bool, bool),
        full_range: bool,
    ) -> Result<(), Error> {
        // MUSTFIX: Do we want bool or something else?
        let (sampling_horiz, sampling_vert) = subsampling;
        fn to_int(b: bool) -> i32 {
            if b {
                1
            } else {
                0
            }
        }

        let result = unsafe {
            ffi::mux::mux_set_color(
                self.segment_ptr(),
                track.0,
                bit_depth.into(),
                to_int(sampling_horiz),
                to_int(sampling_vert),
                to_int(full_range),
            )
        };

        match result {
            RESULT_OK => Ok(()),
            _ => Err(Error::Unknown),
        }
    }

    pub fn finalize(mut self, duration: Option<u64>) -> Result<W, W> {
        let segment_ptr = self.ffi.take().unwrap();

        let result =
            unsafe { ffi::mux::finalize_segment(segment_ptr.as_ptr(), duration.unwrap_or(0)) };

        unsafe {
            ffi::mux::delete_segment(segment_ptr.as_ptr());
        }

        let writer = self.writer.take().unwrap();

        if result == RESULT_OK {
            Ok(writer)
        } else {
            Err(writer)
        }
    }

    fn segment_ptr(&mut self) -> ffi::mux::SegmentMutPtr {
        self.ffi.unwrap().as_ptr()
    }
}

impl<W> Drop for Segment<W> {
    fn drop(&mut self) {
        if let Some(segment_ptr) = self.ffi.take() {
            unsafe {
                ffi::mux::delete_segment(segment_ptr.as_ptr());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mux::writer::Writer;

    use super::*;
    use std::io::Cursor;

    #[test]
    fn bad_track_number() {
        // MUSTFIX: This is because track numbers greater than 126 are not supported by the muxer
        // MUSTFIX: Our types should reflect this
        let mut output = Vec::new();
        let writer = Writer::new(Cursor::new(&mut output));
        let mut segment = Segment::new(writer).expect("Segment should create OK");
        let video_track = segment.add_video_track(420, 420, Some(123456), VideoCodecId::VP8);
        assert!(video_track.is_err());
    }
}
