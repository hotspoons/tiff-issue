use std::{fs, env};
use std::io::{Cursor, BufWriter};
use std::num::NonZeroU32;
use std::time::SystemTime;
use fax::{VecWriter, decoder, decoder::pels, BitWriter, Bits, Color};

use image::{ImageError, ImageEncoder, ColorType, DynamicImage};
use image::codecs::png::PngEncoder;
use image::io::Reader as ImageReader;
use fast_image_resize as fr;


pub struct Resize{
    pub w: i32,
    pub h: i32
}

fn resize_dyn_img_to_png_bytes(img: DynamicImage, framing: &Resize) -> Result<Vec<u8>, ImageError> {
    let now: SystemTime = SystemTime::now();
    let mut image_data = BufWriter::new(Cursor::new(Vec::new()));
            
    if framing.h > 0 || framing.w > 0{
        let mut h = framing.h as u32;
        let mut w = framing.w as u32;
        if framing.h <= 0{
            h = (((img.height() as f32) / (img.width() as f32)) * framing.w as f32).ceil() as u32;
        }
        else if framing.w <= 0{
            w = (((img.width() as f32) / (img.height() as f32)) * framing.h as f32).ceil() as u32;
        }
        let mut src_image = fr::Image::from_vec_u8(
            NonZeroU32::new(img.width()).unwrap(),
            NonZeroU32::new(img.height()).unwrap(),
            img.to_rgba8().into_raw(),
            fr::PixelType::U8x4,
        ).unwrap();
        let alpha_mul_div = fr::MulDiv::default();
        alpha_mul_div
            .multiply_alpha_inplace(&mut src_image.view_mut())
            .unwrap();
        let mut dst_image = fr::Image::new(
            NonZeroU32::new(w).unwrap(),
            NonZeroU32::new(h).unwrap(),
            src_image.pixel_type(),
        );
        let mut dst_view = dst_image.view_mut();
        let mut resizer = fr::Resizer::new(
            fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3),
        );
        resizer.resize(&src_image.view(), &mut dst_view).unwrap();
        // Divide RGB channels of destination image by alpha
        alpha_mul_div.divide_alpha_inplace(&mut dst_view).unwrap();
        PngEncoder::new(&mut image_data).write_image(
            dst_image.buffer(),
            w,
            h,
            ColorType::Rgba8,
        ).unwrap();
        
    }
    else{
        img.write_to(&mut image_data, image::ImageOutputFormat::Png).unwrap();
    }
    match now.elapsed() {
        Ok(elapsed) => {
            println!("{} ms (img conversion and scaling time)", elapsed.as_millis());
        }
        Err(e) => {
            println!("Error: {e:?}");
        }
    }
    let response = image_data.get_ref().get_ref().to_owned();
    return Ok(response);
}


fn image_preprocess_fax(bytes:Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let width = 2560;
    let mut writer = VecWriter::new();
    let mut height = 0;
    decoder::decode_g4(bytes.iter().cloned(), width, None,  |transitions| {
        for c in pels(transitions, width) {
            let bit = match c {
                Color::Black => Bits { data: 1, len: 1 },
                Color::White => Bits { data: 0, len: 1 }
            };
            writer.write(bit);
        }
        writer.pad();
        height += 1;
    });
    let data = writer.finish();
    Ok(data)
}

pub fn read_bytes_to_png_bytes(mut bytes:Vec<u8>, framing: &Resize) -> Result<Vec<u8>, ImageError>{
    if bytes.len() > 3 && bytes[0] == 73 && bytes[1] == 73 // big endian, fine for POC
        && bytes[2] == 42{
            bytes = image_preprocess_fax(bytes).unwrap();
            // TODO reconstruct a tiff or other image representation using the decompressed bitmap
        }

    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;

    match reader.decode(){
        Ok(img) => {
          return resize_dyn_img_to_png_bytes(img, framing);
        }
        Err(e) => {
            eprint!("Error decoding image: {}", e);
            return Err(e);
        }
    };
}

pub fn read_image_to_png_bytes(path: &String, framing: &Resize) -> Result<Vec<u8>, ImageError>{
    let bytes = fs::read(path).unwrap();
    return read_bytes_to_png_bytes(bytes, framing);
}


fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1{
        let path = &args[1];
        let image_bytes = read_image_to_png_bytes(path, &Resize { w: 1280, h:1650 }).unwrap();
        println!("Image resized, {} bytes in final PNG representation", image_bytes.len());
    }
    else{
        println!("Usage - this_executable <path/to/input-image.any>");
    }
}
