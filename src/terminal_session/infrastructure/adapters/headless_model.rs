/// The headless terminal model the session mirrors the shell's screen into.
pub(crate) type Model = alacritty_terminal::term::Term<alacritty_terminal::event::VoidListener>;
