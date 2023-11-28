use colour_utils::Colour;

struct ANSIColours {
    black: Colour,
    red: Colour,
    green: Colour,
    yellow: Colour,
    blue: Colour,
    magenta: Colour,
    cyan: Colour,
    white: Colour,
}

pub struct Colourscheme {
    background: Colour,
    foreground: Colour,

    cursor_bg: Colour,
    cursor_fg: Colour,

    selection_bg: Colour,
    selection_fg: Colour,

    regular: ANSIColours,
    bright: ANSIColours,
}

impl Colourscheme {
    // TODO: parse colour from toml file
    pub fn from_toml(toml: &str) -> Colourscheme {
        Colourscheme{
            background: Colour::from_rgb(0, 0, 0),
            foreground: Colour::from_rgb(255, 255, 255),

            cursor_bg: Colour::from_rgb(255, 255, 255),
            cursor_fg: Colour::from_rgb(0, 0, 0),

            selection_bg: Colour::from_rgb(255, 255, 255),
            selection_fg: Colour::from_rgb(0, 0, 0),

            regular: ANSIColours {
                black: Colour::from_rgb(0, 0, 0),
                red: Colour::from_rgb(255, 0, 0),
                green: Colour::from_rgb(0, 255, 0),
                yellow: Colour::from_rgb(255, 255, 0),
                blue: Colour::from_rgb(0, 0, 255),
                magenta: Colour::from_rgb(255, 0, 255),
                cyan: Colour::from_rgb(0, 255, 255),
                white: Colour::from_rgb(255, 255, 255),
            },

            bright: ANSIColours {
                black: Colour::from_rgb(128, 128, 128),
                red: Colour::from_rgb(255, 0, 0),
                green: Colour::from_rgb(0, 255, 0),
                yellow: Colour::from_rgb(255, 255, 0),
                blue: Colour::from_rgb(0, 0, 255),
                magenta: Colour::from_rgb(255, 0, 255),
                cyan: Colour::from_rgb(0, 255, 255),
                white: Colour::from_rgb(255, 255, 255),
            },
        }

    }
}
