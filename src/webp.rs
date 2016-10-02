use std::mem::{self, transmute};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::ops::Deref;

use surface::{Surface, Yuv420p};
use super::conversions::{
    yuyv_interleave_to_yuv422p,
    yuv422p_from_buffer_mut,
    downsample_yuyv_420p,
    downsample_uv_plane_422p_420p,
};


use webp_sys::{
    WebPConfig,
    WebPConfigInitInternal,
    WebPValidateConfig,
    WebPPicture,
    WebPPictureInitInternal,
    WebPPictureAlloc,
    Enum_WebPPreset,
    WebPMemoryWriter,
    WebPMemoryWriterInit,
    Enum_WebPEncCSP,
    WebPMemoryWrite,
    WebPEncode,
    WebPPictureFree,
};


pub fn reencode<S>(yuv: &Surface<Yuv420p, u8, S>)
    -> Vec<u8>
    where
        S: Deref<Target=[u8]>
{
    let (y_p, u_p, v_p) = yuv.get_planes();
    let mut output = vec![0; yuv.width() as usize * yuv.height() as usize * 4];

    unsafe {
        let mut config: WebPConfig = mem::zeroed();
        assert_eq!(1, WebPConfigInitInternal(
            &mut config as *mut _, Enum_WebPPreset::WEBP_PRESET_PICTURE,
            70.0, 0x0202));
        assert_eq!(1, WebPValidateConfig(&mut config as *mut _));

        let mut pic: WebPPicture = mem::zeroed();
        assert_eq!(1, WebPPictureInitInternal(&mut pic as *mut _, 0x0202));
        pic.width = yuv.width() as i32;
        pic.height = yuv.height() as i32;
        assert_eq!(1, WebPPictureAlloc(&mut pic as *mut _));

        pic.use_argb = 0;
        pic.colorspace = Enum_WebPEncCSP::WEBP_YUV420;
        pic.y = transmute(y_p.as_ptr());
        pic.y_stride = yuv.width() as i32;
        pic.u = transmute(u_p.as_ptr());
        pic.v = transmute(v_p.as_ptr());
        pic.uv_stride = yuv.width() as i32 / 2;

        let mut writer: WebPMemoryWriter = mem::zeroed();
        WebPMemoryWriterInit(&mut writer as *mut _);
        writer.mem = output.as_mut_ptr();
        writer.max_size = output.len() as u64;
        pic.writer = Some(WebPMemoryWrite);
        pic.custom_ptr = &mut writer as *mut WebPMemoryWriter as *mut ::std::os::raw::c_void;
        assert_eq!(1, WebPEncode(&mut config as *mut _, &mut pic as *mut _));
        output.truncate(writer.size as usize);
        assert_eq!(output.len() as u64, writer.size);
        WebPPictureFree(&mut pic as *mut _);
    }

    output
}

