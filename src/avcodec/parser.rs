use crate::{
    avcodec::{AVCodecContext, AVCodecID, AVPacket},
    error::*,
    ffi,
    shared::*,
};

wrap!(AVCodecParserContext: ffi::AVCodecParserContext);

impl AVCodecParserContext {
    /// Allocate a [`AVCodecParserContext`] with given [`AVCodecID`].
    pub fn init(codec_id: AVCodecID) -> Option<Self> {
        // `av_parser_init()` took a plain `int` parameter before FFmpeg 9,
        // which changed it to `enum AVCodecID`. On non-MSVC targets the
        // enum's underlying type is c_uint, which doesn't match the `int`
        // parameter on FFmpeg < 9, so a cast is needed there.
        // ref: https://github.com/rust-lang/rust-bindgen/issues/1361
        #[cfg(all(not(feature = "ffmpeg9"), not(target_env = "msvc")))]
        let codec_id = codec_id as i32;
        unsafe { ffi::av_parser_init(codec_id) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    /// Parse a packet.
    ///
    /// Return `Err(_)` On failure, `bool` field of returned tuple means if
    /// packet is ready, `usize` field of returned tuple means the offset of the
    /// data being parsed.
    ///
    /// Note: if `data.len()` exceeds [`i32::MAX`], this function returns [`RsmpegError::TryFromIntError`].
    pub fn parse_packet(
        &mut self,
        codec_context: &mut AVCodecContext,
        packet: &mut AVPacket,
        data: &[u8],
    ) -> Result<(bool, usize)> {
        let mut packet_data = packet.data;
        let mut packet_size = packet.size;
        let offset = unsafe {
            ffi::av_parser_parse2(
                self.as_mut_ptr(),
                codec_context.as_mut_ptr(),
                &mut packet_data,
                &mut packet_size,
                data.as_ptr(),
                data.len().try_into()?,
                packet.pts,
                packet.dts,
                packet.pos,
            )
        }
        .upgrade()?;
        unsafe {
            packet.deref_mut().data = packet_data;
            packet.deref_mut().size = packet_size;
        }
        Ok((packet.size != 0, offset as usize))
    }
}

impl Drop for AVCodecParserContext {
    fn drop(&mut self) {
        unsafe { ffi::av_parser_close(self.as_mut_ptr()) }
    }
}
