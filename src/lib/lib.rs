extern crate webm_sys as ffi;

pub mod mux {
    use crate::ffi;

    mod segment;
    mod writer;

    pub use ffi::mux::TrackNum;
    pub use {segment::Segment, writer::Writer};

    // MUSTFIX: Needed?
    /// The trait used by [`Segment`] to actually write out WebM data. This is implemented
    /// by [`Writer`], which in most cases is what you actually want to use.
    ///
    /// ## Safety
    /// See the documentation for [`MkvWriter::mkv_writer`].
    #[doc(hidden)]
    pub unsafe trait MkvWriter {
        /// Return the writer that should be passed to `libwebm` when initializing the segment.
        ///
        /// ## Safety
        /// The returned pointer must be non-null and remain valid for the lifetime of the [`Segment`].
        fn mkv_writer(&self) -> ffi::mux::WriterMutPtr;
    }

    // MUSTFIX: Assert numbers are [1, 126] as per Matroska limitations
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct VideoTrackNum(ffi::mux::TrackNum);

    impl VideoTrackNum {
        pub fn as_track_number(&self) -> TrackNum {
            self.0
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct AudioTrackNum(ffi::mux::TrackNum);

    impl AudioTrackNum {
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

    // MUSTFIX
    #[derive(Debug)]
    #[non_exhaustive]
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
}
