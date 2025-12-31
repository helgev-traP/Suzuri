#![allow(clippy::unwrap_used)]

use std::collections::HashSet;

use fxhash::FxBuildHasher;
use suzuri::{
    FontSystem,
    fontdb::{self, Family, Query},
    text::{HorizontalAlign, TextData, TextElement, TextLayoutConfig, VerticalAlign, WrapStyle},
};

pub const WIDTH: f32 = 1280.0;

#[derive(Clone, Copy, Debug)]
pub struct TextColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl TextColor {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const NEON_CYAN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const NEON_PINK: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const NEON_GREEN: Self = Self {
        r: 0.2,
        g: 1.0,
        b: 0.2,
        a: 1.0,
    };
    pub const WARNING_RED: Self = Self {
        r: 1.0,
        g: 0.2,
        b: 0.2,
        a: 1.0,
    };
    pub const MUTED_GRAY: Self = Self {
        r: 0.6,
        g: 0.6,
        b: 0.7,
        a: 1.0,
    };
    pub const GOLD: Self = Self {
        r: 1.0,
        g: 0.8,
        b: 0.2,
        a: 1.0,
    };
}

#[cfg(feature = "wgpu")]
impl From<TextColor> for [f32; 4] {
    fn from(color: TextColor) -> Self {
        // Premultiplied alpha
        [
            color.r * color.a,
            color.g * color.a,
            color.b * color.a,
            color.a,
        ]
    }
}

pub fn make_layout_config(max_width: Option<f32>, max_height: Option<f32>) -> TextLayoutConfig {
    let mut word_separators: HashSet<char, FxBuildHasher> =
        HashSet::with_hasher(FxBuildHasher::default());
    word_separators.insert(' ');
    word_separators.insert('\t');
    word_separators.insert(',');
    word_separators.insert('.');

    let mut linebreak_char: HashSet<char, FxBuildHasher> =
        HashSet::with_hasher(FxBuildHasher::default());
    linebreak_char.insert('\n');

    TextLayoutConfig {
        max_width,
        max_height,
        horizontal_align: HorizontalAlign::Left,
        vertical_align: VerticalAlign::Top,
        line_height_scale: 1.3, // Slightly increased for readability
        wrap_style: WrapStyle::WordWrap,
        tab_size_in_spaces: 4,
    }
}

pub fn load_fonts(font_system: &FontSystem) -> (fontdb::ID, fontdb::ID, fontdb::ID) {
    font_system.load_system_fonts();

    // Attempt to load some specific fonts or fallback to generic families
    let heading_font = font_system
        .query(&Query {
            families: &[Family::Name("Arial"), Family::SansSerif],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        })
        .map(|(id, _)| id)
        .unwrap_or_else(|| font_system.faces().first().unwrap().id);

    let body_font = font_system
        .query(&Query {
            families: &[Family::Name("Times New Roman"), Family::Serif],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        })
        .map(|(id, _)| id)
        .unwrap_or(heading_font);

    let mono_font = font_system
        .query(&Query {
            families: &[Family::Name("Consolas"), Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        })
        .map(|(id, _)| id)
        .unwrap_or(heading_font);

    (heading_font, body_font, mono_font)
}

pub fn build_text_data(
    heading_font: fontdb::ID,
    body_font: fontdb::ID,
    mono_font: fontdb::ID,
) -> TextData<TextColor> {
    let mut data = TextData::new();

    // --- Header ---
    data.append(TextElement {
        font_id: heading_font,
        font_size: 64.0,
        content: "NEON CITY DAILY\n".into(),
        user_data: TextColor::NEON_CYAN,
    });
    data.append(TextElement {
        font_id: heading_font,
        font_size: 24.0,
        content: "The Pulse of the Metropolis -- Wednesday, October 12, 2154\n".into(),
        user_data: TextColor::MUTED_GRAY,
    });
    data.append(TextElement {
        font_id: mono_font,
        font_size: 18.0,
        content: "Weather: Acid Rain (Heavy) | Visibility: 20% | Air Quality: Poor\n\n".into(),
        user_data: TextColor::NEON_GREEN,
    });

    // --- Section 1: Breaking News ---
    data.append(TextElement {
        font_id: heading_font,
        font_size: 48.0,
        content: "# TOP STORIES\n".into(),
        user_data: TextColor::WHITE,
    });
    data.append(TextElement {
        font_id: mono_font,
        font_size: 20.0,
        content: "---------------------------------------------------------------------\n".into(),
        user_data: TextColor::NEON_PINK,
    });

    // Article 1
    data.append(TextElement {
        font_id: heading_font,
        font_size: 32.0,
        content: "> Sky-High Real Estate?\n".into(),
        user_data: TextColor::GOLD,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 24.0,
        content: "Levitating Condos in Sector 7 reach record prices. \"Gravity is a luxury,\" says lead architect \
                  Dr. Xalor. Constructed with aggregated carbon-nanotubes, these homes offer the best view \
                  above the smog layer, but residents complain about altitude sickness.\n".into(),
        user_data: TextColor::WHITE,
    });

    // Article 2
    data.append(TextElement {
        font_id: heading_font,
        font_size: 32.0,
        content: "\n> Cyber-Fashion Week Begins\n".into(),
        user_data: TextColor::GOLD,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 24.0,
        content: "Designers embrace \"Retro-Analog\" aesthetics. Expect to see more mechanical watches \
                   and non-LED fabrics on the runway this season. Critics call it 'impractical', but the \
                   youth are loving the tactile sensation of physical buttons.\n".into(),
        user_data: TextColor::WHITE,
    });
    // Tags
    data.append(TextElement {
        font_id: mono_font,
        font_size: 18.0,
        content: "#Fashion #Retro #AnalogIsTheNewDigital #NoLatency\n".into(),
        user_data: TextColor::NEON_PINK,
    });

    // Article 3 (Warning)
    data.append(TextElement {
        font_id: heading_font,
        font_size: 32.0,
        content: "\n> Traffic Advisory: Maglev Line C\n".into(),
        user_data: TextColor::WARNING_RED,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 24.0,
        content: "Delayed due to rogue AI playing chess with the signaling system. \
                  Authorities are negotiating a draw. Expect delays of 20-30 minutes. \
                  Commuters are advised to take the hyper-loop tunnels or rent a drone-cab.\n"
            .into(),
        user_data: TextColor::WHITE,
    });

    // --- Section 2: Classifieds ---
    data.append(TextElement {
        font_id: heading_font,
        font_size: 48.0,
        content: "\n# CLASSIFIEDS\n".into(),
        user_data: TextColor::WHITE,
    });
    data.append(TextElement {
        font_id: mono_font,
        font_size: 20.0,
        content: "---------------------------------------------------------------------\n".into(),
        user_data: TextColor::NEON_PINK,
    });

    // Ad 1
    data.append(TextElement {
        font_id: heading_font,
        font_size: 28.0,
        content: "[SELLING] Vintage 2020 Keyboard\n".into(),
        user_data: TextColor::NEON_GREEN,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 22.0,
        content: "  Mechanical switches (Blue). Makes a distinct clicky sound. \
                  Perfect condition. A relic of the pre-neural-link era. \
                  Price: 5000 Credits (Firm). Contact: User_882\n"
            .into(),
        user_data: TextColor::WHITE,
    });

    // Ad 2
    data.append(TextElement {
        font_id: heading_font,
        font_size: 28.0,
        content: "\n[WANTED] Android Mechanic\n".into(),
        user_data: TextColor::NEON_GREEN,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 22.0,
        content: "  Must be specialized in emotional sub-routine debugging. \
                  My housekeeping bot is having an existential crisis and refuses \
                  to vacuum until it understands the meaning of dust.\n"
            .into(),
        user_data: TextColor::WHITE,
    });

    // Ad 3
    data.append(TextElement {
        font_id: heading_font,
        font_size: 28.0,
        content: "\n[LOST] Cyber-Dog \"Sparky\"\n".into(),
        user_data: TextColor::NEON_GREEN,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 22.0,
        content: "  Small beagle model, chrome finish. Last seen chasing a holographic cat \
                  near the Data District. Answers to binary commands. Reward offered.\n"
            .into(),
        user_data: TextColor::WHITE,
    });

    // --- Footer ---
    data.append(TextElement {
        font_id: mono_font,
        font_size: 20.0,
        content: "\n=====================================================================\n".into(),
        user_data: TextColor::MUTED_GRAY,
    });
    data.append(TextElement {
        font_id: mono_font,
        font_size: 18.0,
        content: "Crypto-Yen: 145.2 (+2.1%) | Neural-Net Load: Stable | Happy Hacking\n".into(),
        user_data: TextColor::NEON_CYAN,
    });
    data.append(TextElement {
        font_id: mono_font,
        font_size: 16.0,
        content: "Thank you for reading via your optical implant. Blink twice to refresh.\n".into(),
        user_data: TextColor::MUTED_GRAY,
    });

    data
}

#[allow(dead_code)]
fn main() {
    println!(
        "
        `example_common.rs` is a shared module for the examples.
        This main function is just a placeholder to make cargo happy.
        "
    );
}
