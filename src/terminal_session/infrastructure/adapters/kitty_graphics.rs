//! The kitty graphics protocol (SPEC §7.4.2): which terminals take it, and the escape sequences that send, place and
//! clear row icons.

use std::fmt::Write;

use crate::dropdown::domain::value_objects::graphics_support::GraphicsSupport;
use crate::dropdown::domain::value_objects::icon_id::IconId;

/// Base64 payload per kitty chunk (a multiple of 4, the protocol's limit).
const CHUNK: usize = 4096;

/// What a kitty placement shows.
pub(crate) struct KittyImage {
    pub(crate) id: u32,
    pub(crate) src: KittyPixels,
}

pub(crate) enum KittyPixels {
    Png(&'static [u8]),
    /// 32×32 RGBA8.
    Rgba(Vec<u8>),
}

/// Kitty graphics protocol is used for Ghostty and kitty only. Inside tmux, tmux tells the outer terminal and whether
/// it passes sequences through (`allow-passthrough`, off by default).
pub(crate) fn detect() -> GraphicsSupport {
    let var = |k: &str| std::env::var(k).ok();
    if var("TMUX").is_none() {
        let on = var("KITTY_WINDOW_ID").is_some()
            || var("TERM_PROGRAM").as_deref() == Some("ghostty")
            || var("TERM").as_deref() == Some("xterm-kitty");
        return if on { GraphicsSupport::Direct } else { GraphicsSupport::Off };
    }
    let mut cmd = std::process::Command::new("tmux");
    cmd.args(["display", "-p"]);
    if let Some(pane) = var("TMUX_PANE") {
        cmd.args(["-t", &pane]);
    }
    let out = cmd
        .arg("#{allow-passthrough} #{client_termname} #{client_termtype}")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    tmux_graphics(&out)
}

/// `allow-passthrough client_termname client_termtype` → Tmux when passthrough is on and the client is kitty or
/// Ghostty (by `TERM` or by its XTVERSION reply, e.g. `kitty(0.47.1)`).
fn tmux_graphics(out: &str) -> GraphicsSupport {
    match out.trim().split_once(' ') {
        Some((pass, client))
            if matches!(pass, "on" | "all")
                && (client.contains("kitty") || client.contains("ghostty")) =>
        {
            GraphicsSupport::Tmux
        }
        _ => GraphicsSupport::Off,
    }
}

/// Kitty graphics: transmit `img` as image `id` without displaying it. `q=2` keeps the terminal from answering on
/// stdin. Payloads go in 4096-byte base64 chunks, `m=1` while more follow. Behind tmux each chunk goes through the
/// passthrough, and the first one also makes the slot-sized virtual placement the placeholders show (`U=1`).
pub(crate) fn transmit(img: &KittyImage, g: GraphicsSupport) -> String {
    let (format, data) = match &img.src {
        KittyPixels::Png(b) => ("f=100", *b),
        KittyPixels::Rgba(b) => ("f=32,s=32,v=32", b.as_slice()),
    };
    let enc = base64(data);
    let mut out = String::new();
    for (k, chunk) in enc.as_bytes().chunks(CHUNK).enumerate() {
        let more = u8::from((k + 1) * CHUNK < enc.len());
        let chunk = std::str::from_utf8(chunk).unwrap_or_default();
        let apc = if k == 0 {
            let action = if g == GraphicsSupport::Tmux {
                "a=T,U=1,c=2,r=1"
            } else {
                "a=t"
            };
            format!(
                "\x1b_G{action},i={},{format},q=2,m={more};{chunk}\x1b\\",
                img.id
            )
        } else {
            format!("\x1b_Gm={more};{chunk}\x1b\\")
        };
        if g == GraphicsSupport::Tmux {
            let _ = write!(out, "\x1bPtmux;{}\x1b\\", apc.replace('\x1b', "\x1b\x1b"));
        } else {
            out += &apc;
        }
    }
    out
}

/// Kitty graphics: show `icon` at the cursor, one cell row by two columns, under the text (`z=-1`). `p` tells the
/// placements of one frame apart; `C=1` keeps the cursor where it is.
pub(crate) fn place(icon: IconId, p: usize) -> String {
    format!("\x1b_Ga=p,i={},p={p},c=2,r=1,C=1,z=-1,q=2\x1b\\", icon.value())
}

/// Kitty graphics: remove every placement on screen. Image data stays, so it is not sent again.
pub(crate) const CLEAR_PLACEMENTS: &str = "\x1b_Ga=d,d=a,q=2\x1b\\";

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = u32::from(c[0]) << 16
            | u32::from(c.get(1).copied().unwrap_or(0)) << 8
            | u32::from(c.get(2).copied().unwrap_or(0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
    }

    #[test]
    fn transmit_chunks() {
        let small = transmit(
            &KittyImage {
                id: 7,
                src: KittyPixels::Png(b"Man"),
            },
            GraphicsSupport::Direct,
        );
        assert_eq!(small, "\x1b_Ga=t,i=7,f=100,q=2,m=0;TWFu\x1b\\");
        // 3072 bytes → 4096 base64 chars: one chunk; 4000 bytes → 5336 chars: two chunks.
        let big = transmit(
            &KittyImage {
                id: 1,
                src: KittyPixels::Rgba(vec![0; 4000]),
            },
            GraphicsSupport::Direct,
        );
        let parts: Vec<&str> = big.split("\x1b\\").filter(|s| !s.is_empty()).collect();
        assert_eq!(parts.len(), 2);
        assert!(
            parts[0].starts_with("\x1b_Ga=t,i=1,f=32,s=32,v=32,q=2,m=1;"),
            "{}",
            &parts[0][..40]
        );
        assert!(parts[1].starts_with("\x1b_Gm=0;"));
    }

    #[test]
    fn tmux_uses_placeholders_passthrough() {
        // Every chunk is wrapped for tmux, ESC doubled inside.
        let t = transmit(
            &KittyImage {
                id: 7,
                src: KittyPixels::Png(b"Man"),
            },
            GraphicsSupport::Tmux,
        );
        assert_eq!(
            t,
            "\x1bPtmux;\x1b\x1b_Ga=T,U=1,c=2,r=1,i=7,f=100,q=2,m=0;TWFu\x1b\x1b\\\x1b\\"
        );
        // Detection from `tmux display`.
        assert_eq!(
            tmux_graphics("all xterm-kitty kitty(0.47.1)\n"),
            GraphicsSupport::Tmux
        );
        assert_eq!(
            tmux_graphics("on tmux-256color ghostty 1.1.3\n"),
            GraphicsSupport::Tmux
        );
        assert_eq!(
            tmux_graphics("off xterm-kitty kitty(0.47.1)\n"),
            GraphicsSupport::Off
        );
        assert_eq!(tmux_graphics(" xterm-256color iTerm2 3.5\n"), GraphicsSupport::Off);
        assert_eq!(tmux_graphics(""), GraphicsSupport::Off);
    }
}
