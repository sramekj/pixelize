use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{env, fs};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub number_of_colors: Option<usize>,
    pub sample_factor: Option<i32>,
    pub desired_width: Option<u32>,
    pub desired_height: Option<u32>,
    pub uniform_scale_by_width: bool,
    pub uniform_scale_by_height: bool,
    pub use_custom_palette: bool,
    pub custom_palette: Vec<(u8, u8, u8)>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            number_of_colors: Some(8),
            sample_factor: Some(10),
            desired_width: Some(32),
            desired_height: Some(32),
            uniform_scale_by_width: false,
            uniform_scale_by_height: false,
            use_custom_palette: false,
            custom_palette: vec![],
        }
    }
}

const CFG_FILENAME: &str = "config.toml";

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path(CFG_FILENAME)?;
        let cfg = if !config_path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            cfg
        } else {
            toml::from_str::<Config>(&fs::read_to_string(config_path.as_path())?)?
        };
        Ok(cfg)
    }

    fn get_config_path(filename: &str) -> Result<PathBuf> {
        let exe_path = env::current_exe()?;
        let exe_dir = exe_path.parent().unwrap();
        Ok(exe_dir.join(filename))
    }

    fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path(CFG_FILENAME)?;
        fs::write(config_path.as_path(), toml::to_string(&self)?)?;
        Ok(())
    }

    fn validate(&self) -> Vec<String> {
        let mut validation_messages = vec![];
        if !self.use_custom_palette {
            if self.number_of_colors.is_none() || self.sample_factor.is_none() {
                validation_messages.push("Warning: invalid configuration: number_of_colors and sample_factor are missing.".to_string());
            }
        } else if self.custom_palette.is_empty() {
            validation_messages
                .push("Warning: invalid configuration: custom_palette is empty.".to_string());
        };
        if !self.uniform_scale_by_width
            && !self.uniform_scale_by_height
            && (self.desired_width.is_none() || self.desired_height.is_none())
        {
            validation_messages.push(
                "Warning: invalid configuration: desired_width and desired_height are missing."
                    .to_string(),
            );
        }
        if !self.uniform_scale_by_width && self.desired_width.is_none() {
            validation_messages
                .push("Warning: invalid configuration: desired_width is missing.".to_string());
        }
        if !self.uniform_scale_by_height && self.desired_height.is_none() {
            validation_messages
                .push("Warning: invalid configuration: desired_height is missing.".to_string());
        }
        validation_messages
    }

    pub fn is_valid(&self) -> bool {
        let errors = self.validate();
        errors.iter().for_each(|e| eprintln!("{}", e));
        errors.is_empty()
    }
}
