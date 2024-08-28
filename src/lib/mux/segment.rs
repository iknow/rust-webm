use core::ptr::NonNull;

use super::{AudioCodecId, AudioTrackNum, Error, MkvWriter, TrackNum, VideoCodecId, VideoTrackNum};
use ffi::mux::{TrackNum as RawTrackNum, RESULT_OK};

// MUSTFIX: If we move the drop impl, can we avoid Option?
/// A Matroska segment. This is where tracks are created and frames are written.
///
/// In typical usage, you first create a [`Writer`](crate::mux::Writer), use that to create a single segment, and go
/// from there.
///
/// ## Finalization
/// Once you are done writing frames to this segment, you must call [`Segment::finalize`] on it.
/// This performs a few final writes, and the resulting WebM may not be playable without it.
/// Notably, for memory safety reasons, just dropping a [`Segment`] will not finalize it!
pub struct Segment<W> {
    ffi: Option<ffi::mux::SegmentNonNullPtr>,
    writer: Option<W>,
}

// SAFETY: `libwebm` does not contain thread-locals or anything that would violate `Send`-safety.
// Thus, safety is only conditional on the write destination `W`, hence the `Send` bound on it.
//
// `libwebm` is not thread-safe, however, which is why we do not implement `Sync`.
unsafe impl<W: Send> Send for Segment<W> {}

impl<W> Segment<W> {
    /// Creates a new Matroska segment that writes WebM data to `dest`.
    /// This `dest` parameter typically is a [`Writer`](crate::mux::Writer).
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

    /// Sets the name of the muxing application. This will become the `MuxingApp` element of the resulting
    /// WebM.
    ///
    /// Calling this after the first frame has been written has no effect.
    pub fn set_muxing_app_name(&mut self, name: &str) {
        let name = std::ffi::CString::new(name).unwrap();
        unsafe {
            ffi::mux::mux_set_writing_app(self.segment_ptr(), name.as_ptr());
        }
    }

    /// Adds a new video track to this segment, returning its track number.
    ///
    /// You may request a specific track number using the `track_num` parameter. If one is specified, and this method
    /// succeeds, the returned track number is guaranteed to match the requested one. If a track with that number
    /// already exists, however, this method will fail. Leave as `None` to allow an available number to be chosen for
    /// you.
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn add_video_track(
        &mut self,
        width: u32,
        height: u32,
        track_num: Option<TrackNum>,
        codec: VideoCodecId,
    ) -> Result<VideoTrackNum, Error> {
        let mut track_num_out: RawTrackNum = 0;
        let desired_track_num: RawTrackNum = track_num.map(|n| n.0.into()).unwrap_or(0);

        let result = unsafe {
            ffi::mux::segment_add_video_track(
                self.segment_ptr(),
                // MUSTFIX
                width as i32,
                height as i32,
                desired_track_num.try_into().unwrap(),
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            RESULT_OK => {
                let result_track_num = TrackNum::try_from_raw(track_num_out).unwrap();

                // If a specific track number was requested, make sure we got it
                if let Some(desired) = track_num {
                    assert_eq!(desired, result_track_num);
                }

                Ok(VideoTrackNum(result_track_num))
            }
            _ => Err(Error::Unknown),
        }
    }

    /// Adds a new audio track to this segment, returning its track number.
    ///
    /// You may request a specific track number using the `track_num` parameter. If one is specified, and this method
    /// succeeds, the returned track number is guaranteed to match the requested one. If a track with that number
    /// already exists, however, this method will fail. Leave as `None` to allow an available number to be chosen for
    /// you.
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn add_audio_track(
        &mut self,
        sample_rate: i32,
        channels: i32,
        track_num: Option<TrackNum>,
        codec: AudioCodecId,
    ) -> Result<AudioTrackNum, Error> {
        let mut track_num_out: RawTrackNum = 0;
        let desired_track_num: RawTrackNum = track_num.map(|n| n.0.into()).unwrap_or(0);

        let result = unsafe {
            ffi::mux::segment_add_audio_track(
                self.segment_ptr(),
                sample_rate,
                channels,
                desired_track_num.try_into().unwrap(),
                codec.get_id(),
                &mut track_num_out,
            )
        };

        match result {
            RESULT_OK => {
                let result_track_num = TrackNum::try_from_raw(track_num_out).unwrap();

                // If a specific track number was requested, make sure we got it
                if let Some(desired) = track_num {
                    assert_eq!(desired, result_track_num);
                }

                Ok(AudioTrackNum(result_track_num))
            }
            _ => Err(Error::Unknown),
        }
    }

    /// Adds a frame to the track with the specified track number. If you have a [`VideoTrackNum`] or
    /// [`AudioTrackNum`], you can call `as_track_number()` to get the underlying [`TrackNum`].
    ///
    /// The timestamp must be in nanosecond units, and must be monotonically increasing with respect to all other
    /// timestamps written so far, including those of other tracks! Repeating the last written timestamp is allowed,
    /// however players generally don't handle this well if both such frames are on the same track.
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
                track_num.into_raw(),
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

    /// Sets the CodecPrivate data a frame to the track with the specified track number. If you have a
    /// [`VideoTrackNum`] or [`AudioTrackNum`], you can call `as_track_number()` to get the underlying [`TrackNum`].
    ///
    /// This method will fail if called after the first frame has been written.
    pub fn set_codec_private(&mut self, track_number: TrackNum, data: &[u8]) -> Result<(), Error> {
        let result = unsafe {
            ffi::mux::segment_set_codec_private(
                self.segment_ptr(),
                track_number.into_raw(),
                data.as_ptr(),
                data.len().try_into().unwrap(),
            )
        };

        match result {
            RESULT_OK => Ok(()),
            _ => Err(Error::Unknown),
        }
    }

    /// Sets color information for the specified video track.
    ///
    /// This method will fail if called after the first frame has been written.
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
                track.as_track_number().into_raw(),
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

    /// Finalizes the segment and consumes it, returning the underlying writer. Note that the finalizing process will
    /// itself trigger writes (such as to write seeking information).
    ///
    /// The resulting WebM may not be playable if you drop the [`Segment`] without calling this first!
    ///
    /// You may specify an explicit `duration` to be written to the segment's `Duration` element. However, this requires
    /// seeking and thus will be ignored if the writer was not created with [`Seek`](std::io::Seek) support.
    ///
    /// Finalization is known to fail if no frames have been written.
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
        // Do _not_ attempt to finalize the segment! This will cause writes, and we have no guarantee the writer
        // is still valid (it may have been dropped simultaneously)
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
    fn overlapping_track_number() {
        let mut output = Vec::new();
        let writer = Writer::new(Cursor::new(&mut output));
        let mut segment = Segment::new(writer).expect("Segment should create OK");
        let track_num = TrackNum::try_from(123).unwrap();

        let video_track = segment.add_video_track(420, 420, Some(track_num), VideoCodecId::VP8);
        assert!(video_track.is_ok());

        let video_track = segment.add_video_track(420, 420, Some(track_num), VideoCodecId::VP8);
        assert!(video_track.is_err());
    }
}
