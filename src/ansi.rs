use regex::{Regex, Captures};
use lazy_static::lazy_static;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Styles {
    background: Option<u8>,
    blink: bool,
    bold: bool,
    color: Option<u8>,
    inverse: bool,
    italic: bool,
    strikethrough: bool,
    underline: bool
}

impl Styles {
    fn new() -> Self {
        Styles {
            background: None,
            blink: false,
            bold: false,
            color: None,
            inverse: false,
            italic: false,
            strikethrough: false,
            underline: false
        }
    }
}

pub fn ansi2html(str: &str) -> String {
    lazy_static! {
        static ref ANSI_COLORS: Regex = Regex::new("(?:\x1B\\[\\d+(?:;\\d+)*m)+").unwrap();
    }
    let mut styles = Styles::new();
    
    let mut first = true;
    let mut result = ANSI_COLORS.replace_all(str, |caps: &Captures| -> String {
        lazy_static! {
            static ref CODE_REGEX: Regex = Regex::new(r"\d+").unwrap();
        }
        let codes = CODE_REGEX
            .find_iter(caps.get(0).unwrap().as_str())
            .filter_map(|s| s.as_str().parse::<u8>().ok());

        let mut newstyles = styles.clone();
        for code in codes {
            match code {
                0 => newstyles = Styles::new(),
                1 => newstyles.bold = true,
                3 => newstyles.italic = true,
                4 | 21 => newstyles.underline = true,
                5 | 6 => newstyles.blink = true,
                7 => newstyles.inverse = true,
                9 => newstyles.strikethrough = true,
                2 | 22 => newstyles.bold = false,
                23 => newstyles.italic = false,
                24 => newstyles.underline = false,
                25 => newstyles.blink = false,
                27 => newstyles.inverse = false,
                29 => newstyles.strikethrough = false,
                30..=37 => newstyles.color = Some(code - 30),
                39 => newstyles.color = None,
                40..=47 => newstyles.background = Some(code - 40),
                49 => newstyles.background = None,
                _ => ()
            }
        }

        if newstyles == styles {
            return String::new();
        }

        styles = newstyles;

        let mut html = String::with_capacity(32);
        if !first {
            html.push_str("</span>");
        } else {
            first = false;
        }
        html.push_str(r#"<span class=""#);
        
        if let Some(background) = styles.background {
            html.push_str(BACKGROUNDS[background as usize]);
        }

        if styles.blink {
            html.push_str("tnc_blink ");
        }

        if styles.inverse {
            html.push_str("tnc_inverse ");
        }

        if styles.strikethrough {
            html.push_str("tnc_line_through ");
        }

        if styles.underline {
            html.push_str("tnc_underline ");
        }

        if styles.bold {
            html.push_str("tnc_bold ");
        }

        if let Some(color) = styles.color {
            html.push_str(COLORS[color as usize]);
        }

        if styles.italic {
            html.push_str("tnc_italic ");
        }

        if Some(&b' ') == html.as_bytes().last() {
            html.pop();
        }

        html.push_str(r#"">"#);

        html
    }).to_string();
    result.push_str("</span>");
    result
}

static BACKGROUNDS: [&'static str; 8] = ["tnc_bg_black ", "tnc_bg_red ", "tnc_bg_green ", "tnc_bg_yellow ", "tnc_bg_blue ", "tnc_bg_magenta ", "tnc_bg_cyan ", "tnc_bg_silver "];
static COLORS: [&'static str; 8] = ["tnc_black ","tnc_red ","tnc_green ","tnc_yellow ","tnc_blue ","tnc_magenta ","tnc_cyan ","tnc_white "];