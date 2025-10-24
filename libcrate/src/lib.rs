use crate::image_processing::{
    apply_palette, generate_image_palette, get_color_histogram, save_image, scale,
};
use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::{ImageReader, Rgb, RgbImage};
use std::collections::HashMap;
use std::path::Path;

pub type RgbHistogram = HashMap<Rgb<u8>, u32>;
pub type Palette = Vec<Rgb<u8>>;

pub struct ProcessedImage {
    pub data: RgbImage,
}

impl ProcessedImage {
    pub fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let data = ImageReader::open(path.as_ref())
            .with_context(|| format!("Failed to open: {:?}", path.as_ref()))?
            .decode()
            .with_context(|| "Failed to decode the file")?
            .to_rgb8();
        Ok(ProcessedImage { data })
    }

    pub fn from_buffer(width: u32, height: u32, buffer: &[Rgb<u8>]) -> Self {
        let mut data = RgbImage::new(width, height);
        let mut idx = 0;
        data.pixels_mut().for_each(|px| {
            *px = buffer[idx];
            idx += 1;
        });
        ProcessedImage { data }
    }

    pub fn get_color_histogram(&self) -> RgbHistogram {
        get_color_histogram(&self.data)
    }

    pub fn generate_image_palette(&self, sample_factor: i32, number_of_colors: usize) -> Palette {
        generate_image_palette(&self.data, sample_factor, number_of_colors)
    }

    pub fn apply_palette(&mut self, palette: &Palette) {
        self.data = apply_palette(&self.data, palette);
    }

    pub fn scale(&mut self, new_width: u32, new_height: u32, smooth: bool) {
        self.data = scale(
            &self.data,
            new_width,
            new_height,
            if smooth {
                FilterType::Lanczos3
            } else {
                FilterType::Nearest
            },
        );
    }

    pub fn uniform_scale_width(&mut self, new_width: u32, smooth: bool) {
        let (width, height) = self.data.dimensions();
        let ratio = new_width as f64 / width as f64;
        let new_height = (height as f64 * ratio) as u32;
        self.scale(new_width, new_height, smooth);
    }

    pub fn uniform_scale_height(&mut self, new_height: u32, smooth: bool) {
        let (width, height) = self.data.dimensions();
        let ratio = new_height as f64 / height as f64;
        let new_width = (width as f64 * ratio) as u32;
        self.scale(new_width, new_height, smooth);
    }

    pub fn save<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        save_image(path.as_ref(), &self.data)
    }

    pub fn width(&self) -> u32 {
        self.data.width()
    }

    pub fn height(&self) -> u32 {
        self.data.height()
    }
}

pub mod image_processing {
    use crate::{Palette, RgbHistogram};
    use anyhow::{Context, Result};
    use color_quant::NeuQuant;
    use image::imageops::FilterType;
    use image::{Rgb, RgbImage};
    use kiddo::{KdTree, SquaredEuclidean};
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::path::Path;

    type Point = [f64; 3];

    pub fn get_color_histogram(data: &RgbImage) -> RgbHistogram {
        data.pixels()
            .par_bridge()
            .fold(HashMap::new, |mut local_map, pixel| {
                *local_map.entry(*pixel).or_insert(0) += 1;
                local_map
            })
            .reduce(HashMap::new, |mut map1, map2| {
                for (k, v) in map2 {
                    *map1.entry(k).or_insert(0) += v;
                }
                map1
            })
    }

    pub fn generate_image_palette(
        data: &RgbImage,
        sample_factor: i32,
        number_of_colors: usize,
    ) -> Palette {
        let pixels: Vec<u8> = data.pixels().flat_map(|p| p.0.to_vec()).collect();
        let quantizer = NeuQuant::new(sample_factor, number_of_colors, &pixels);
        let color_map = quantizer.color_map_rgb();
        color_map
            .chunks(3)
            .map(|c| Rgb([c[0], c[1], c[2]]))
            .collect()
    }

    fn rgb_to_point(rgb: &Rgb<u8>) -> Point {
        [rgb[0] as f64, rgb[1] as f64, rgb[2] as f64]
    }

    pub fn apply_palette(img: &RgbImage, palette: &Palette) -> RgbImage {
        let mut tree: KdTree<f64, 3> = KdTree::new();
        let mut color_map = HashMap::new();
        for (i, color) in palette.iter().enumerate() {
            let item = i as u64;
            tree.add(&rgb_to_point(color), item);
            color_map.insert(item, *color);
        }
        let (width, height) = img.dimensions();
        let processed_pixels: Vec<(u32, u32, Rgb<u8>)> = img
            .enumerate_pixels()
            .par_bridge()
            .map(|(x, y, pixel)| {
                let point = rgb_to_point(pixel);
                let nearest = tree.nearest_one::<SquaredEuclidean>(&point);
                let nearest_color = color_map[&nearest.item];
                (x, y, nearest_color)
            })
            .collect();
        let mut new_img = RgbImage::new(width, height);
        for (x, y, color) in processed_pixels {
            new_img.put_pixel(x, y, color);
        }
        new_img
    }

    pub fn scale(img: &RgbImage, new_width: u32, new_height: u32, filter: FilterType) -> RgbImage {
        image::imageops::resize(img, new_width, new_height, filter)
    }

    pub fn save_palette<P>(path: P, palette: &Palette) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let size = palette.len() as u32;
        let mut result = RgbImage::new(size, 1);
        let mut idx = 0;
        result.pixels_mut().for_each(|p| {
            *p = palette[idx];
            idx += 1;
        });
        result
            .save(path.as_ref())
            .with_context(|| "Failed to save image")?;
        Ok(())
    }

    pub fn save_image<P>(path: P, data: &RgbImage) -> Result<()>
    where
        P: AsRef<Path>,
    {
        data.save(path)?;
        Ok(())
    }

    pub fn palette_from_tuples(tuples: &[(u8, u8, u8)]) -> Palette {
        tuples
            .iter()
            .copied()
            .map(|(r, g, b)| Rgb([r, g, b]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::ProcessedImage;
    use crate::image_processing::save_palette;
    use image::Rgb;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    #[allow(dead_code)]
    fn generate_img_code<P>(path: P, output: P)
    where
        P: AsRef<Path>,
    {
        let image = ProcessedImage::new(path).unwrap();
        let pixels = image.data.pixels().map(|p| p.0).collect::<Vec<_>>();
        let data_str = pixels
            .iter()
            .map(|p| format!("Rgb([{},{},{}])", p[0], p[1], p[2]))
            .collect::<Vec<_>>()
            .join(", ");
        fs::write(output, format!("let img_data = [{}];", data_str)).unwrap();
    }

    fn get_test_image() -> ProcessedImage {
        let img_data = [
            Rgb([136u8, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([185, 122, 87]),
            Rgb([185, 122, 87]),
            Rgb([34, 177, 76]),
            Rgb([34, 177, 76]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([185, 122, 87]),
            Rgb([185, 122, 87]),
            Rgb([34, 177, 76]),
            Rgb([34, 177, 76]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([185, 122, 87]),
            Rgb([185, 122, 87]),
            Rgb([34, 177, 76]),
            Rgb([34, 177, 76]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([185, 122, 87]),
            Rgb([185, 122, 87]),
            Rgb([34, 177, 76]),
            Rgb([34, 177, 76]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([255, 242, 0]),
            Rgb([185, 122, 87]),
            Rgb([185, 122, 87]),
            Rgb([34, 177, 76]),
            Rgb([34, 177, 76]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([136, 0, 21]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
            Rgb([63, 72, 204]),
        ];
        ProcessedImage::from_buffer(10, 10, &img_data)
    }

    #[test]
    fn test_histogram() {
        let image = get_test_image();
        println!("Dimensions: {}x{}", image.width(), image.height());
        let histogram = image.get_color_histogram();
        println!("Histogram: {:?}", histogram);
        assert_eq!(histogram.len(), 6);
        let expected = HashMap::from([
            (Rgb([136, 0, 21]), 30),
            (Rgb([185, 122, 87]), 10),
            (Rgb([255, 242, 0]), 15),
            (Rgb([34, 177, 76]), 10),
            (Rgb([63, 72, 204]), 21),
            (Rgb([0, 0, 0]), 14),
        ]);
        assert_eq!(histogram, expected);
    }

    #[test]
    fn test_scaling() {
        let mut image = get_test_image();
        image.scale(100, 100, true);
        assert_eq!(image.width(), 100);
        assert_eq!(image.height(), 100);
        image.uniform_scale_width(50, true);
        assert_eq!(image.width(), 50);
        assert_eq!(image.height(), 50);
        image.uniform_scale_height(70, true);
        assert_eq!(image.width(), 70);
        assert_eq!(image.height(), 70);
    }

    #[test]
    fn test_palette_gen() {
        let image = get_test_image();
        let palette = image.generate_image_palette(10, 6);
        let expected_palette = [
            Rgb([239, 0, 180]),
            Rgb([117, 21, 17]),
            Rgb([7, 27, 143]),
            Rgb([171, 171, 171]),
            Rgb([93, 177, 123]),
            Rgb([213, 213, 213]),
        ];
        assert_eq!(palette, expected_palette);
    }

    #[test]
    fn test_apply_palette() {
        let buffer = [
            Rgb([0xFFu8, 0xFF, 0xFF]),
            Rgb([0x88, 0x88, 0x88]),
            Rgb([0x22, 0x22, 0x22]),
            Rgb([0, 0, 0]),
        ];
        let mut image = ProcessedImage::from_buffer(2, 2, &buffer);
        let palette = vec![Rgb([0u8, 0, 0]), Rgb([0x90, 0x90, 0x90])];
        image.apply_palette(&palette);
        let expected = [0x90u8, 0x90, 0x90, 0x90, 0x90, 0x90, 0, 0, 0, 0, 0, 0]
            .into_iter()
            .collect::<Vec<_>>();
        let data = image.data.as_raw();
        assert_eq!(data, &expected);
    }

    #[test]
    #[ignore]
    fn end_to_end() {
        {
            let mut image = ProcessedImage::new("./assets/test_img_1.jpg").unwrap();
            println!("Dimensions: {}x{}", image.width(), image.height());
            let orig_width = image.width();
            image.uniform_scale_width(orig_width / 5, true);
            let palette = image.generate_image_palette(10, 16);
            println!("Palette: {:?}", palette);
            save_palette("./assets/palette1.png", &palette).unwrap();
            image.apply_palette(&palette);
            image.uniform_scale_width(orig_width, false);
            image.save("./assets/converted1.png").unwrap();
        }
        {
            let mut image = ProcessedImage::new("./assets/test_img_1.jpg").unwrap();
            println!("Dimensions: {}x{}", image.width(), image.height());
            let orig_width = image.width();
            image.uniform_scale_width(orig_width / 2, true);
            let palette = image.generate_image_palette(10, 8);
            image.apply_palette(&palette);
            image.save("./assets/converted3.png").unwrap();
        }
        {
            let mut image = ProcessedImage::new("./assets/test_img_2.jpg").unwrap();
            println!("Dimensions: {}x{}", image.width(), image.height());
            let orig_width = image.width();
            image.uniform_scale_width(orig_width / 5, true);
            let palette = image.generate_image_palette(10, 16);
            println!("Palette: {:?}", palette);
            save_palette("./assets/palette2.png", &palette).unwrap();
            image.apply_palette(&palette);
            image.uniform_scale_width(orig_width, false);
            image.save("./assets/converted2.png").unwrap();
        }
        {
            let mut image = ProcessedImage::new("./assets/test_img_2.jpg").unwrap();
            println!("Dimensions: {}x{}", image.width(), image.height());
            let orig_width = image.width();
            image.uniform_scale_width(orig_width / 2, true);
            let palette = image.generate_image_palette(10, 8);
            image.apply_palette(&palette);
            image.save("./assets/converted4.png").unwrap();
        }
    }
}
