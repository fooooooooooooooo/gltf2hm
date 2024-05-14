use anstyle::AnsiColor;
use clap::builder::Styles;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None, styles = styles())]
pub struct Args {
  /// Input file path
  pub input: String,
  /// Output file path
  #[clap(short, long)]
  pub output: Option<String>,

  /// Export BeamNG .ter file
  #[clap(long, default_value_t = true)]
  pub beamng: bool,
  /// Export heightmap .png file
  #[clap(long, default_value_t = true)]
  pub heightmap: bool,

  /// Resolution of the heightmap
  #[clap(short, long, default_value_t = 2048)]
  pub size: usize,

  /// Flip the heightmap on the X axis
  #[clap(long, default_value_t = false)]
  pub flip_x: bool,
  /// Flip the heightmap on the Y axis
  #[clap(long, default_value_t = false)]
  pub flip_y: bool,

  /// Smoothing tolerance
  #[clap(long, default_value_t = 0.0)]
  pub smooth: f32,
}

pub fn styles() -> Styles {
  Styles::styled()
    .header(AnsiColor::Green.on_default())
    .usage(AnsiColor::Green.on_default())
    .literal(AnsiColor::BrightBlue.on_default())
    .placeholder(AnsiColor::White.on_default())
    .invalid(AnsiColor::Red.on_default())
    .error(AnsiColor::Red.on_default())
    .valid(AnsiColor::Green.on_default())
}
