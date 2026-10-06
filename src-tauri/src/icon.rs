use tauri::image::Image;
pub fn icon()->Image<'static>{Image::new_owned(vec![220u8;32*32*4],32,32)}