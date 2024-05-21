use crate::view::color::RGBColor;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Colors {
    #[default]
    Default, // default/background
    Focused,    // default/alt background
    Inverted,   // background/default
    Insert,     // white/green
    Warning,    // white/yellow
    PathMode,   // white/pink
    SearchMode, // white/purple
    SelectMode, // white/blue
    CustomForeground(RGBColor),
    CustomFocusedForeground(RGBColor),
    Custom(RGBColor, RGBColor),
}
