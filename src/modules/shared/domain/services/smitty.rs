/// Smitty, the tabsmith mascot: an orange tabby jazz cat in a bow tie, named for the alto saxophonist Tab Smith, drawn
/// in plain ASCII so every terminal can show him. Setup ends with him playing his sax and saying hi.
pub(crate) fn welcome() -> &'static str {
    r"   /\_M_/\         ~ ~
  ( o   o )       ~        Hi, I'm Smitty!
 =(   w   )=o_     __      I'll improvise completions as you type.
  /  >o<  \  \\   //
 (_________)  \\_//
   (_)   (_)   \_/
"
}

/// Uninstall ends with Smitty waving goodbye, sax down.
pub(crate) fn farewell() -> &'static str {
    r#"   /\_M_/\   \ | /
  ( ^   ^ )   (")      Bye for now!
 =(   w   )=  //       Catch you at the next set.
  /  >o<  \__//
 (_________)
   (_)   (_)
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    /// After setup Smitty, in his bow tie, plays a few notes on his alto sax and says hi.
    #[test]
    fn should_introduce_smitty_playing_his_sax() {
        let expected = [
            r"   /\_M_/\         ~ ~",
            r"  ( o   o )       ~        Hi, I'm Smitty!",
            r" =(   w   )=o_     __      I'll improvise completions as you type.",
            r"  /  >o<  \  \\   //",
            r" (_________)  \\_//",
            r"   (_)   (_)   \_/",
            "",
        ]
        .join("\n");

        let drawing = welcome();

        assert_eq!(drawing, expected);
    }

    /// After uninstall Smitty puts the sax down and waves goodbye with his paw up, as a set ends.
    #[test]
    fn should_wave_goodbye_with_his_paw_up() {
        let expected = [
            r"   /\_M_/\   \ | /",
            r#"  ( ^   ^ )   (")      Bye for now!"#,
            r" =(   w   )=  //       Catch you at the next set.",
            r"  /  >o<  \__//",
            r" (_________)",
            r"   (_)   (_)",
            "",
        ]
        .join("\n");

        let drawing = farewell();

        assert_eq!(drawing, expected);
    }
}
