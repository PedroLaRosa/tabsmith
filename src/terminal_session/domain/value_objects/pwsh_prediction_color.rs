use crate::terminal_session::domain::value_objects::shell_suggestion_color::{
    rgb_to_256, ShellSuggestionColor,
};
use crate::terminal_session::domain::value_objects::text_emphasis::TextEmphasis;

/// PSReadLine's `InlinePredictionColor`: the colours of a prediction plus the dim/italic a prediction cell must carry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PwshPredictionColor {
    colors: ShellSuggestionColor,
    emphasis: TextEmphasis,
}

impl PwshPredictionColor {
    pub(crate) fn colors(&self) -> &ShellSuggestionColor {
        &self.colors
    }

    pub(crate) fn emphasis(&self) -> TextEmphasis {
        self.emphasis
    }

    /// A cell drawn in `fg` on `bg` with `cell` emphasis is a prediction: the colours match and it carries all the
    /// required emphasis.
    pub(crate) fn matches(&self, fg: Option<u8>, bg: Option<u8>, cell: TextEmphasis) -> bool {
        self.colors.matches(fg, bg) && self.emphasis.is_within(cell)
    }

    /// SGR parameters of a pwsh `InlinePredictionColor` (ESC stripped: `[97;2;3m`, `38;5;8`, `[38;2;r;g;bm[3m`): its
    /// colours plus the DIM/ITALIC a prediction cell must carry. None when it sets neither.
    pub(crate) fn parse(spec: &str) -> Option<PwshPredictionColor> {
        let p: Vec<u16> = spec
            .split([';', ':', '[', 'm'])
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        let byte = |k: usize| p.get(k).and_then(|&n| u8::try_from(n).ok());
        let (mut fg, mut bg): (Vec<Option<u8>>, Vec<Option<u8>>) = (Vec::new(), Vec::new());
        let (mut dim, mut italic, mut i) = (false, false, 0);
        while i < p.len() {
            // 38/48;5;n and 38/48;2;r;g;b: (colour, parameters used).
            let ext = match p.get(i + 1) {
                Some(5) => (byte(i + 2), 3),
                Some(2) => (
                    (|| Some(rgb_to_256(byte(i + 2)?, byte(i + 3)?, byte(i + 4)?)))(),
                    5,
                ),
                _ => (None, 1),
            };
            let mut step = 1;
            match p[i] {
                0 => (fg, bg, dim, italic) = (Vec::new(), Vec::new(), false, false),
                2 => dim = true,
                3 => italic = true,
                n @ 30..=37 => fg = vec![Some(n as u8 - 30)],
                n @ 90..=97 => fg = vec![Some(n as u8 - 90 + 8)],
                39 => fg = vec![None],
                n @ 40..=47 => bg = vec![Some(n as u8 - 40)],
                n @ 100..=107 => bg = vec![Some(n as u8 - 100 + 8)],
                49 => bg = vec![None],
                n @ (38 | 48) => {
                    step = ext.1;
                    if let Some(col) = ext.0 {
                        *(if n == 38 { &mut fg } else { &mut bg }) = vec![Some(col)];
                    }
                }
                _ => {}
            }
            i += step;
        }
        let emphasis = TextEmphasis::new(dim, italic);
        (!fg.is_empty() || !bg.is_empty() || !emphasis.is_empty()).then(|| PwshPredictionColor {
            colors: ShellSuggestionColor::new(fg, bg),
            emphasis,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_require_every_reported_emphasis_besides_the_colours() {
        let reported = PwshPredictionColor {
            colors: ShellSuggestionColor::new(vec![Some(8)], vec![]),
            emphasis: TextEmphasis::new(true, true),
        };

        assert!(reported.matches(Some(8), None, TextEmphasis::new(true, true)));
        assert!(!reported.matches(Some(8), None, TextEmphasis::new(true, false)));
        assert!(!reported.matches(Some(9), None, TextEmphasis::new(true, true)));
    }

    #[test]
    fn pwsh_reported_prediction_colour_sgr() {
        let p = |s: &str| {
            PwshPredictionColor::parse(s).map(|c| {
                (
                    c.colors().fg().to_vec(),
                    c.colors().bg().to_vec(),
                    c.emphasis(),
                )
            })
        };
        assert_eq!(
            p("[97;2;3m"),
            Some((vec![Some(15)], vec![], TextEmphasis::new(true, true)))
        );
        assert_eq!(
            p("38;5;238;48;2;0;0;0"),
            Some((vec![Some(238)], vec![Some(16)], TextEmphasis::default()))
        );
        assert_eq!(
            p("[38;2;170;170;170m[3m"),
            Some((
                vec![Some(rgb_to_256(170, 170, 170))],
                vec![],
                TextEmphasis::new(false, true)
            ))
        );
        assert_eq!(
            p("[39;49m"),
            Some((vec![None], vec![None], TextEmphasis::default()))
        );
        assert_eq!((p("[0m"), p("[1;4m"), p("x")), (None, None, None));
    }
}
