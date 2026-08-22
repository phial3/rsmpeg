use anyhow::{Context, Result};
use image::{ImageFormat, RgbImage};
use rsmpeg::{
    avfilter::{AVFilter, AVFilterGraph, AVFilterInOut},
    avutil::AVFrame,
    ffi,
};
use std::{
    ffi::CStr,
    fs::{self},
    path::Path,
};

fn frame_copy_to_buffer(filter_spec: &CStr, output_image_path: impl AsRef<Path>) -> Result<()> {
    let frame = get_libav_allocated_frame(filter_spec)?;
    debug_assert_eq!(frame.format, ffi::AV_PIX_FMT_RGB24);

    let buffer_size = frame.image_get_buffer_size(1)?;
    let mut buffer = vec![0u8; buffer_size];
    let written = frame.image_copy_to_buffer(&mut buffer, 1)?;
    assert_eq!(buffer_size, written);

    write_out_rgb24(
        buffer,
        frame.width as u32,
        frame.height as u32,
        output_image_path,
    )?;

    Ok(())
}

// Use AVFilter to generate test frames (i.e. we cannot create an AVImage to access the data that way)
fn get_libav_allocated_frame(filter_spec: &CStr) -> Result<AVFrame> {
    let testsrc2_filter =
        AVFilter::get_by_name(c"testsrc2").context("could not find testsrc2 filter")?;
    let buffersink_filter =
        AVFilter::get_by_name(c"buffersink").context("could not find buffersink filter")?;

    let filter_graph = AVFilterGraph::new();

    let mut testsrc2_ctx = filter_graph.create_filter_context(
        &testsrc2_filter,
        c"in",
        Some(c"size=800x600:rate=30"),
    )?;

    let mut buffersink_ctx = filter_graph
        .alloc_filter_context(&buffersink_filter, c"out")
        .context("could not allocate buffersink context")?;
    // FFmpeg 7.1 renamed the buffersink option `pix_fmts` (int-list binary
    // option) to `pixel_formats` (array-type option), and the deprecated old
    // one was removed in FFmpeg 8+.
    #[cfg(not(feature = "ffmpeg7_1"))]
    buffersink_ctx.opt_set_bin(c"pix_fmts", &ffi::AV_PIX_FMT_RGB24)?;
    #[cfg(feature = "ffmpeg7_1")]
    buffersink_ctx.opt_set_array(
        c"pixel_formats",
        0,
        Some(&[ffi::AV_PIX_FMT_RGB24]),
        ffi::AV_OPT_TYPE_PIXEL_FMT,
    )?;
    buffersink_ctx.init_dict(&mut None)?;

    let outputs = AVFilterInOut::new(c"in", &mut testsrc2_ctx, 0);
    let inputs = AVFilterInOut::new(c"out", &mut buffersink_ctx, 0);

    let (_inputs, _outputs) = filter_graph.parse_ptr(filter_spec, Some(inputs), Some(outputs))?;

    filter_graph.config()?;

    let frame = buffersink_ctx.buffersink_get_frame(None)?;
    println!("Frame info: {:#?}", frame);

    Ok(frame)
}

fn write_out_rgb24(
    pixel_values: Vec<u8>,
    width: u32,
    height: u32,
    output_image_path: impl AsRef<Path>,
) -> Result<()> {
    let image = RgbImage::from_raw(width, height, pixel_values)
        .context("Can't create rgb image from buffer")?;

    fs::create_dir_all(
        output_image_path
            .as_ref()
            .parent()
            .context("could not get output parent dir")?,
    )?;
    image.save_with_format(output_image_path, ImageFormat::Png)?;

    Ok(())
}

#[test]
fn test_frame_copy_to_buffer0() {
    frame_copy_to_buffer(c"null", "tests/output/frame_copy_to_buffer/sink.png").unwrap();
    // Compare decoded pixels instead of raw file bytes: the PNG encoder of the
    // `image` dev-dependency is not pinned (Cargo.lock is not tracked), so its
    // output bytes may change between versions while pixels stay identical.
    let output = image::open("tests/output/frame_copy_to_buffer/sink.png")
        .unwrap()
        .to_rgb8();
    let expect = image::open("tests/assets/pics/sink.png").unwrap().to_rgb8();
    assert_eq!(output, expect);
}
