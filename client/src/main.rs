mod config;

use crate::config::Config;
use anyhow::{Result, anyhow};
use clap::{ArgGroup, Parser};
use libcrate::ProcessedImage;
use libcrate::image_processing::{palette_from_tuples, save_palette};

#[derive(Parser, Debug)]
#[command(author, version, about)]
#[command(group(
    ArgGroup::new("input")
        .args(["input_pos", "input_flag"])
        .required(true)
))]
#[command(group(
    ArgGroup::new("output")
        .args(["output_pos", "output_flag"])
        .required(true)
))]
struct Args {
    #[arg(short = 'i', long = "input", group = "input")]
    input_flag: Option<String>,
    #[arg(short = 'o', long = "output", group = "output")]
    output_flag: Option<String>,
    #[arg(index = 1, group = "input")]
    input_pos: Option<String>,
    #[arg(index = 2, group = "output")]
    output_pos: Option<String>,
}

fn main() -> Result<()> {
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let args = Args::parse();

    let input = args
        .input_flag
        .or(args.input_pos)
        .expect("Missing input file");
    let output = args
        .output_flag
        .or(args.output_pos)
        .expect("Missing output file");

    let config = Config::load()?;
    if !config.is_valid() {
        return Err(anyhow!("Config is not valid."));
    }
    println!("Loading image...");
    let mut image = ProcessedImage::new(input)?;

    if config.uniform_scale_by_width {
        println!("Uniform scaling by width...");
        image.uniform_scale_width(config.desired_width.unwrap(), true);
    } else if config.uniform_scale_by_height {
        println!("Uniform scaling by height...");
        image.uniform_scale_height(config.desired_height.unwrap(), true);
    } else if config.desired_width.is_none() && config.desired_height.is_none() {
        println!("Skipping scaling");
    } else {
        println!("Scaling by width and height...");
        image.scale(
            config.desired_width.unwrap(),
            config.desired_height.unwrap(),
            true,
        );
    }

    let palette = if config.use_custom_palette {
        println!("Using custom palette...");
        palette_from_tuples(&config.custom_palette)
    } else {
        println!("Generating palette...");
        image.generate_image_palette(
            config.sample_factor.unwrap(),
            config.number_of_colors.unwrap(),
        )
    };

    if config.dump_palette {
        println!("Saving palette to palette.png");
        save_palette("./palette.png", &palette)?;
    }

    println!("Applying palette...");
    image.apply_palette(&palette);

    println!("Saving to {}", output);
    image.save(&output)?;

    println!("Done.");
    Ok(())
}
