pub static INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
pub static INTER_BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold.ttf");
pub static INTER_ITALIC: &[u8] = include_bytes!("../fonts/Inter-Italic.ttf");
pub static INTER_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Inter-BoldItalic.ttf");
pub static JETBRAINS_MONO_REGULAR: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");

pub static ALL_FONTS: &[&[u8]] = &[
    INTER_REGULAR,
    INTER_BOLD,
    INTER_ITALIC,
    INTER_BOLD_ITALIC,
    JETBRAINS_MONO_REGULAR,
];
