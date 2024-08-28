extern crate webm_sys as ffi;

pub mod mux {
    use core::num::NonZeroU64;

    mod segment;
    mod writer;

    pub use {segment::Segment, writer::Writer};

    /// The Matroska-level track number. Typically this is wrapped in [`VideoTrackNum`] or [`AudioTrackNum`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TrackNum(NonZeroU64);

    impl TryFrom<u64> for TrackNum {
        /// The only possible error is out-of-range.
        type Error = ();

        fn try_from(value: u64) -> Result<Self, Self::Error> {
            // `libwebm` limitations only allow for a restricted subrange of [1, 126] for track numbers.
            // However, this limitation is not inherent to WebM, and so we keep the underlying type `NonZeroU64`,
            // in accordance with Matroska limits, in case this limitation is later removed
            if value > 126 {
                return Err(());
            }
            let nonzero = NonZeroU64::new(value).ok_or(())?;

            Ok(TrackNum(nonzero))
        }
    }

    impl TrackNum {
        pub(crate) fn try_from_raw(raw: ffi::mux::TrackNum) -> Option<Self> {
            // The track number types used by `libwebm` are inconsistent.
            // Matroska allows for 64-bit numbers, but `libwebm` itself only allows
            // a very restricted subrange of [1, 126].
            TrackNum::try_from(raw).ok()
        }

        pub(crate) fn into_raw(self) -> ffi::mux::TrackNum {
            self.0.into()
        }
    }

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VideoTrackNum(TrackNum);

    impl VideoTrackNum {
        pub fn as_track_number(&self) -> TrackNum {
            self.0
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AudioTrackNum(TrackNum);

    impl AudioTrackNum {
        pub fn as_track_number(&self) -> TrackNum {
            self.0
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// The error type for this entire crate. More specific error types will
    /// be added in the future, hence the current marking as non-exhaustive.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Error {
        /// An unknown error occurred. While this is typically the result of
        /// incorrect parameters to methods, this is not a guarantee.
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
