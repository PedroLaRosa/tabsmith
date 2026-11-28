/// Smitty, the tabsmith mascot: an orange tabby blacksmith, drawn in plain ASCII so every terminal can show him.
/// Setup ends with him saying hi, hammer up.
pub(crate) fn welcome() -> &'static str {
    r"   /\_M_/\   .----.
  ( o   o )  '----'    Hi, I'm Smitty!
 =(   w   )=   ||      I'll forge your completions as you type.
  /[ ->| ]\o___||
 (_________)
   (_)   (_)
"
}

/// Uninstall ends with Smitty waving goodbye, hammer down.
pub(crate) fn farewell() -> &'static str {
    r#"   /\_M_/\   \ | /
  ( ^   ^ )   (")      Bye for now!
 =(   w   )=  //       I'll keep the forge warm for you.
  /[ ->| ]\__//
 (_________)
   (_)   (_)
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    /// After setup Smitty holds up his hammer, with the Tab key on his apron, and says hi.
    #[test]
    fn should_introduce_smitty_holding_his_hammer() {
        let expected = [
            r"   /\_M_/\   .----.",
            r"  ( o   o )  '----'    Hi, I'm Smitty!",
            r" =(   w   )=   ||      I'll forge your completions as you type.",
            r"  /[ ->| ]\o___||",
            r" (_________)",
            r"   (_)   (_)",
            "",
        ]
        .join("\n");

        let drawing = welcome();

        assert_eq!(drawing, expected);
    }

    /// After uninstall Smitty puts the hammer down and waves goodbye with his paw up.
    #[test]
    fn should_wave_goodbye_with_his_paw_up() {
        let expected = [
            r"   /\_M_/\   \ | /",
            r#"  ( ^   ^ )   (")      Bye for now!"#,
            r" =(   w   )=  //       I'll keep the forge warm for you.",
            r"  /[ ->| ]\__//",
            r" (_________)",
            r"   (_)   (_)",
            "",
        ]
        .join("\n");

        let drawing = farewell();

        assert_eq!(drawing, expected);
    }
}
