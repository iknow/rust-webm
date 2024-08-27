extern crate webm_sys as ffi;

pub mod mux {
    pub use ffi::mux::TrackNum;
    use ffi::mux::{WriterGetPosFn, WriterSetPosFn, RESULT_OK};

    use crate::ffi;
    use std::os::raw::c_void;

    use std::io::{Seek, Write};
    use std::pin::Pin;
    use std::ptr::NonNull;

    /// Structure for writing a muxed WebM stream to the user-supplied write destination `T`.
    ///
    /// `T` may be a file, an `std::io::Cursor` over a byte array, or anything implementing the [`Write`] trait.
    /// It is recommended, but not required, that `T` also implement [`Seek`]. This allows the resulting WebM
    /// file to have things like seeking headers and a stream duration known upfront.
    ///
    /// Once this [`Writer`] is created, you can use it to create one or more [`Segment`]s.
    pub struct Writer<T>
    where
        T: Write,
    {
        writer_data: Pin<Box<MuxWriterData<T>>>,
        mkv_writer: ffi::mux::WriterNonNullPtr,
    }

    unsafe impl<T: Send + Write> Send for Writer<T> {}

    struct MuxWriterData<T> {
        dest: T,

        /// Used for tracking position when using a non-Seek write destination
        bytes_written: u64,
    }

    impl<T> Writer<T>
    where
        T: Write,
    {
        /// Creates a [`Writer`] for a destination that does not support [`Seek`].
        /// If it does support [`Seek`], you should use [`Writer::new()`] instead.
        pub fn new_non_seek(dest: T) -> Writer<T> {
            extern "C" fn get_pos_fn<T>(data: *mut c_void) -> u64 {
                // The user-supplied writer does not track its own position.
                // Use our own based on how much has been written
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                data.bytes_written
            }

            Self::make_writer(dest, get_pos_fn::<T>, None)
        }

        /// Consumes this [`Writer`], and returns the user-supplied write destination
        /// that it was created with.
        #[must_use]
        pub fn unwrap(self) -> T {
            unsafe {
                ffi::mux::delete_writer(self.mkv_writer.as_ptr());
                Pin::into_inner_unchecked(self.writer_data).dest
            }
        }

        fn make_writer(
            dest: T,
            get_pos_fn: WriterGetPosFn,
            set_pos_fn: Option<WriterSetPosFn>,
        ) -> Self {
            extern "C" fn write_fn<T>(data: *mut c_void, buf: *const c_void, len: usize) -> bool
            where
                T: Write,
            {
                if buf.is_null() {
                    return false;
                }
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                let buf = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), len) };

                let result = data.dest.write(buf);
                if let Ok(num_bytes) = result {
                    // Guard against a future universe where sizeof(usize) > sizeof(u64)
                    let num_bytes_u64: u64 = num_bytes.try_into().unwrap();

                    data.bytes_written += num_bytes_u64;

                    // Partial writes are considered failure
                    num_bytes == len
                } else {
                    false
                }
            }

            let mut writer_data = Box::pin(MuxWriterData {
                dest,
                bytes_written: 0,
            });
            let mkv_writer = unsafe {
                ffi::mux::new_writer(
                    Some(write_fn::<T>),
                    Some(get_pos_fn),
                    set_pos_fn,
                    None,
                    (writer_data.as_mut().get_unchecked_mut() as *mut MuxWriterData<T>).cast(),
                )
            };
            assert!(!mkv_writer.is_null());

            Writer {
                writer_data,
                mkv_writer: NonNull::new(mkv_writer).unwrap(),
            }
        }
    }

    impl<T> Writer<T>
    where
        T: Write + Seek,
    {
        /// Creates a [`Writer`] for a destination that supports [`Seek`].
        /// If it does not support [`Seek`], you should use [`Writer::new_non_seek()`] instead.
        pub fn new(dest: T) -> Writer<T> {
            use std::io::SeekFrom;

            extern "C" fn get_pos_fn<T>(data: *mut c_void) -> u64
            where
                T: Write + Seek,
            {
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                data.dest.stream_position().unwrap()
            }
            extern "C" fn set_pos_fn<T>(data: *mut c_void, pos: u64) -> bool
            where
                T: Write + Seek,
            {
                let data = unsafe { data.cast::<MuxWriterData<T>>().as_mut().unwrap() };
                data.dest.seek(SeekFrom::Start(pos)).is_ok()
            }

            Self::make_writer(dest, get_pos_fn::<T>, Some(set_pos_fn::<T>))
        }
    }

    #[doc(hidden)]
    pub trait MkvWriter {
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr;
    }

    impl<T> MkvWriter for Writer<T>
    where
        T: Write,
    {
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr {
            self.mkv_writer.as_ptr()
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct VideoTrackId(ffi::mux::TrackNum);

    impl VideoTrackId {
        pub fn as_track_number(&self) -> TrackNum {
            self.0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct AudioTrackId(ffi::mux::TrackNum);

    impl AudioTrackId {
        pub fn as_track_number(&self) -> TrackNum {
            self.0
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    pub enum AudioCodecId {
        Opus,
        Vorbis,
    }

    impl AudioCodecId {
        fn get_id(&self) -> u32 {
            match self {
                AudioCodecId::Opus => ffi::mux::OPUS_CODEC_ID,
                AudioCodecId::Vorbis => ffi::mux::VORBIS_CODEC_ID,
            }
        }
    }

    #[derive(Eq, PartialEq, Clone, Copy, Debug)]
    pub enum VideoCodecId {
        VP8,
        VP9,
        AV1,
    }

    impl VideoCodecId {
        fn get_id(&self) -> u32 {
            match self {
                VideoCodecId::VP8 => ffi::mux::VP8_CODEC_ID,
                VideoCodecId::VP9 => ffi::mux::VP9_CODEC_ID,
                VideoCodecId::AV1 => ffi::mux::AV1_CODEC_ID,
            }
        }
    }

    unsafe impl<W: Send> Send for Segment<W> {}

    // MUSTFIX
    #[derive(Debug)]
    pub enum Error {
        Unknown,
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match *self {
                Error::Unknown => f.write_str("Unknown error"),
            }
        }
    }

    impl std::error::Error for Error {}

    pub struct Segment<W> {
        ffi: Option<ffi::mux::SegmentNonNullPtr>,
        writer: Option<W>,
    }

    impl<W> Segment<W> {
        /// Note: the supplied writer must have a lifetime larger than the segment.
        pub fn new(dest: W) -> Option<Self>
        where
            W: MkvWriter,
        {
            let ffi = unsafe { ffi::mux::new_segment() };
            let ffi = NonNull::new(ffi)?;
            let success = unsafe { ffi::mux::initialize_segment(ffi.as_ptr(), dest.mkv_writer()) };
            if !success {
                return None;
            }

            Some(Segment {
                ffi: Some(ffi),
                writer: Some(dest),
            })
        }

        fn segment_ptr(&self) -> ffi::mux::SegmentNonNullPtr {
            self.ffi.unwrap()
        }

        pub fn set_app_name(&mut self, name: &str) {
            let name = std::ffi::CString::new(name).unwrap();
            let ffi_lock = self.segment_ptr();
            unsafe {
                ffi::mux::mux_set_writing_app(ffi_lock.as_ptr(), name.as_ptr());
            }
        }

        pub fn add_video_track(
            &mut self,
            width: u32,
            height: u32,
            track_num: Option<i32>,
            codec: VideoCodecId,
        ) -> Result<VideoTrackId, Error> {
            // MUSTFIX: Do we really need the ability to dictate track_num?
            let mut id_out: TrackNum = 0;
            let ffi_lock = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_video_track(
                    ffi_lock.as_ptr(),
                    width as i32,
                    height as i32,
                    track_num.unwrap_or(0),
                    codec.get_id(),
                    &mut id_out,
                )
            };

            match result {
                RESULT_OK => {
                    assert_ne!(id_out, 0);
                    Ok(VideoTrackId(id_out))
                }
                _ => Err(Error::Unknown),
            }
        }

        pub fn set_codec_private(&mut self, track_number: TrackNum, data: &[u8]) -> bool {
            let ffi_lock = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_set_codec_private(
                    ffi_lock.as_ptr(),
                    track_number,
                    data.as_ptr(),
                    data.len().try_into().unwrap(),
                )
            };
            result == RESULT_OK
        }

        pub fn add_audio_track(
            &mut self,
            sample_rate: i32,
            channels: i32,
            track_num: Option<i32>,
            codec: AudioCodecId,
        ) -> Result<AudioTrackId, Error> {
            let mut id_out: TrackNum = 0;
            let ffi_lock = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_audio_track(
                    ffi_lock.as_ptr(),
                    sample_rate,
                    channels,
                    track_num.unwrap_or(0),
                    codec.get_id(),
                    &mut id_out,
                )
            };

            match result {
                RESULT_OK => {
                    assert_ne!(id_out, 0);
                    Ok(AudioTrackId(id_out))
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
        ) -> bool {
            // MUSTFIX: We don't need to lock anymore
            let ffi_lock = self.segment_ptr();
            let result = unsafe {
                ffi::mux::segment_add_frame(
                    ffi_lock.as_ptr(),
                    track_num,
                    data.as_ptr(),
                    data.len(),
                    timestamp_ns,
                    keyframe,
                )
            };
            result == RESULT_OK
        }

        pub fn set_color(
            &mut self,
            track: VideoTrackId,
            bit_depth: u8,
            subsampling: (bool, bool),
            full_range: bool,
        ) -> bool {
            // MUSTFIX: Do we want bool or something else?
            let (sampling_horiz, sampling_vert) = subsampling;
            fn to_int(b: bool) -> i32 {
                if b {
                    1
                } else {
                    0
                }
            }

            let ffi_lock = self.segment_ptr();
            let result = unsafe {
                ffi::mux::mux_set_color(
                    ffi_lock.as_ptr(),
                    track.0,
                    bit_depth.into(),
                    to_int(sampling_horiz),
                    to_int(sampling_vert),
                    to_int(full_range),
                )
            };
            result == RESULT_OK
        }

        pub fn finalize(mut self, duration: Option<u64>) -> Result<W, W> {
            let segment_ptr = self.ffi.take().unwrap();

            let result =
                unsafe { ffi::mux::finalize_segment(segment_ptr.as_ptr(), duration.unwrap_or(0)) };

            unsafe {
                ffi::mux::delete_segment(segment_ptr.as_ptr());
            }

            let writer = self.writer.take().unwrap();

            if result {
                Ok(writer)
            } else {
                Err(writer)
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bad_track_number() {
        // MUSTFIX: This is because track numbers greater than 126 are not supported by the muxer
        // MUSTFIX: Our types should reflect this
        let mut output = Vec::new();
        let writer = mux::Writer::new(Cursor::new(&mut output));
        let mut segment = mux::Segment::new(writer).expect("Segment should create OK");
        let video_track = segment.add_video_track(420, 420, Some(123456), mux::VideoCodecId::VP8);
        assert!(video_track.is_err());
    }
}
