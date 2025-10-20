mod config;

use crate::config::Config;
use anyhow::{Result, anyhow};
use clap::Parser;
use libcrate::{Image, Palette, PaletteFromVec};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'f', long)]
    filename: String,
    #[arg(short = 'o', long)]
    output: String,
}

fn main() -> Result<()> {
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let args = Args::parse();
    let config = Config::load()?;
    if !config.is_valid() {
        return Err(anyhow!("Config is not valid."));
    }
    println!("Loading image...");
    let mut image = Image::new(args.filename)?;
    println!("Processing image...");

    let palette = if config.use_custom_palette {
        println!("Using custom palette...");
        Palette::from_vec(config.custom_palette)
    } else {
        println!("Generating palette...");
        image.generate_image_palette(
            config.sample_factor.unwrap(),
            config.number_of_colors.unwrap(),
        )
    };

    println!("Applying palette...");
    image.apply_palette(&palette);

    if config.uniform_scale_by_width {
        println!("Uniform scaling by width...");
        image.uniform_scale_width(config.desired_width.unwrap());
    } else if config.uniform_scale_by_height {
        println!("Uniform scaling by height...");
        image.uniform_scale_height(config.desired_height.unwrap());
    } else {
        println!("Scaling by width and height...");
        image.scale(
            config.desired_width.unwrap(),
            config.desired_height.unwrap(),
        );
    }

    println!("Saving to {}", args.output);
    image.save(&args.output)?;

    println!("Done.");
    Ok(())
}
