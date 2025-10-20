use crate::image_processing::{
    apply_palette, generate_image_palette, get_color_histogram, save_image, scale,
};
use anyhow::{Context, Result};
use image::{ImageReader, Rgb, RgbImage};
use std::collections::HashMap;
use std::path::Path;

pub type RgbHistogram = HashMap<Rgb<u8>, u32>;
pub type Palette = Vec<Rgb<u8>>;

pub struct ProcessedImage {
    data: RgbImage,
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

    pub fn get_color_histogram(&self) -> RgbHistogram {
        get_color_histogram(&self.data)
    }

    pub fn generate_image_palette(&self, sample_factor: i32, number_of_colors: usize) -> Palette {
        generate_image_palette(&self.data, sample_factor, number_of_colors)
    }

    pub fn apply_palette(&mut self, palette: &Palette) {
        self.data = apply_palette(&self.data, palette);
    }

    pub fn scale(&mut self, new_width: u32, new_height: u32) {
        self.data = scale(&self.data, new_width, new_height);
    }

    pub fn uniform_scale_width(&mut self, new_width: u32) {
        let (width, height) = self.data.dimensions();
        let ratio = new_width as f64 / width as f64;
        let new_height = (height as f64 * ratio) as u32;
        self.scale(new_width, new_height);
    }

    pub fn uniform_scale_height(&mut self, new_height: u32) {
        let (width, height) = self.data.dimensions();
        let ratio = new_height as f64 / height as f64;
        let new_width = (width as f64 * ratio) as u32;
        self.scale(new_width, new_height);
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
        let mut new_img = RgbImage::new(width, height);
        for (x, y, pixel) in img.enumerate_pixels() {
            let point = rgb_to_point(pixel);
            let nearest = tree.nearest_one::<SquaredEuclidean>(&point);
            let nearest_color = color_map[&nearest.item];
            new_img.put_pixel(x, y, nearest_color);
        }
        new_img
    }

    pub fn scale(img: &RgbImage, new_width: u32, new_height: u32) -> RgbImage {
        image::imageops::resize(img, new_width, new_height, FilterType::Nearest)
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

    #[test]
    #[ignore]
    fn test_histogram() {
        let image = ProcessedImage::new("./assets/test_img_2.png").unwrap();
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
    #[ignore]
    fn test_palette_and_scaling() {
        let mut image = ProcessedImage::new("./assets/test_img_1.jpg").unwrap();
        println!("Dimensions: {}x{}", image.width(), image.height());
        let palette = image.generate_image_palette(10, 8);
        println!("Palette: {:?}", palette);
        save_palette("./assets/palette.png", &palette).unwrap();
        image.apply_palette(&palette);
        image.uniform_scale_width(100);
        image.save("./assets/converted1.png").unwrap();
    }
}
