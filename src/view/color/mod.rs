extern crate termion;

mod colors;
pub use self::colors::Colors;

mod map;
pub use self::map::ColorMap;

pub use self::termion::color::Rgb as RGBColor;
use syntect::highlighting::Color as RGBAColor;

pub fn to_rgb_color(color: RGBAColor) -> RGBColor {
    RGBColor(color.r, color.g, color.b)
}
