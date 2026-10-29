//! The argv state machine (SPEC §6.4): pure transitions over one spec level, no I/O.
use crate::completion::domain::value_objects::annotation_kind::AnnotationKind;
use crate::completion::domain::value_objects::parsed_command_line::{
    ParsedCommandLine, ParsedCommandLineParts,
};
use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
struct ArgState {
    /// Empty = no arg slot (`args: null`).
    args: Vec<Rc<Arg>>,
    index: usize,
    variadic_count: usize,
}

impl ArgState {
    fn new(args: &[Rc<Arg>]) -> ArgState {
        ArgState {
            args: args.to_vec(),
            ..Default::default()
        }
    }

    fn cur(&self) -> Option<&Rc<Arg>> {
        self.args.get(self.index)
    }

    fn advance(&mut self) {
        match self.cur() {
            Some(a) if a.is_variadic() => self.variadic_count += 1,
            Some(_) if self.index + 1 < self.args.len() => {
                *self = ArgState {
                    args: std::mem::take(&mut self.args),
                    index: self.index + 1,
                    variadic_count: 0,
                }
            }
            _ => *self = ArgState::default(),
        }
    }
}

fn mandatory_or_var(a: Option<&Rc<Arg>>) -> bool {
    a.is_some_and(|a| a.is_variadic() || !a.is_optional())
}

#[derive(Clone, Debug)]
pub(crate) struct ArgvState {
    completion: Rc<Subcommand>,
    path: Vec<Rc<Subcommand>>,
    /// Effective parserDirectives (inherited downwards without mutating the spec).
    directives: ParserDirectives,
    /// Inherited persistent options, lookup order (parent first).
    persistent: Vec<Rc<SpecOption>>,
    opt_args: ArgState,
    sub_args: ArgState,
    annotations: Vec<TokenAnnotation>,
    passed: Vec<Rc<SpecOption>>,
    command_index: usize,
    entered_sub_args: bool,
    end_of_options: bool,
    /// Token texts from this parse's start, after `parserDirectives.alias` substitution (last one is stale when
    /// cached). Callers prepend their own prefix, so a cached nested state never carries another command's prefix.
    tokens: Vec<String>,
    spec_name: String,
}

fn ann(kind: AnnotationKind, text: &str) -> TokenAnnotation {
    TokenAnnotation::new(kind, text.into(), Vec::new())
}

impl ArgvState {
    pub(crate) fn initial(spec: Rc<Subcommand>, text: &str, spec_name: &str, tokens: Vec<String>) -> ArgvState {
        ArgvState {
            path: vec![spec.clone()],
            directives: spec.directives().clone(),
            persistent: spec.persistent().clone(),
            opt_args: ArgState::default(),
            sub_args: ArgState::new(spec.args()),
            annotations: vec![ann(AnnotationKind::Subcommand, text)],
            passed: Vec::new(),
            command_index: 0,
            entered_sub_args: false,
            end_of_options: false,
            tokens,
            spec_name: spec_name.into(),
            completion: spec,
        }
    }

    /// Replace the current subcommand in place (inline loadSpec, generateSpec merge).
    fn set_completion(&mut self, c: Rc<Subcommand>) {
        if let Some(last) = self.path.last_mut() {
            *last = c.clone();
        }
        self.completion = c;
    }

    pub(crate) fn prefer_opt_arg(&self) -> bool {
        mandatory_or_var(self.opt_args.cur()) || self.sub_args.cur().is_none()
    }

    fn arg_state(&self) -> &ArgState {
        if self.prefer_opt_arg() {
            &self.opt_args
        } else {
            &self.sub_args
        }
    }

    fn can_subcommands(&self) -> bool {
        !mandatory_or_var(self.opt_args.cur()) && !self.entered_sub_args
    }

    fn can_options(&self) -> bool {
        if (self.entered_sub_args && self.directives.options_must_precede_args())
            || self.end_of_options
        {
            return false;
        }
        if let Some(o) = self.opt_args.cur().filter(|a| mandatory_or_var(Some(a))) {
            return o.is_variadic()
                && self.opt_args.variadic_count > 0
                && o.options_can_break_variadic() != Some(false);
        }
        !self.sub_args.cur().is_some_and(|s| {
            s.is_variadic()
                && self.sub_args.variadic_count > 0
                && s.options_can_break_variadic() == Some(false)
        })
    }

    /// `options[flag] || persistentOptions[flag]`.
    fn find_option(&self, flag: &str) -> Option<Rc<SpecOption>> {
        match self.completion.opt_index().get(flag) {
            Some(&(false, i)) => self.completion.options().get(i).cloned(),
            _ => self
                .persistent
                .iter()
                .find(|o| o.names().iter().any(|n| n == flag))
                .cloned(),
        }
    }

    pub(crate) fn completion(&self) -> &Rc<Subcommand> {
        &self.completion
    }

    pub(crate) fn annotations(&self) -> &Vec<TokenAnnotation> {
        &self.annotations
    }

    pub(crate) fn current_arg(&self) -> Option<&Rc<Arg>> {
        self.arg_state().cur()
    }

    pub(crate) fn with_tokens(mut self, tokens: Vec<String>) -> ArgvState {
        self.tokens = tokens;
        self
    }

    /// A nested parse's state seen from its caller: the caller's index added, its prefix tokens prepended.
    pub(crate) fn with_command_prefix(mut self, index: usize, prefix: impl Iterator<Item = String>) -> ArgvState {
        self.command_index += index;
        self.tokens = prefix.chain(self.tokens).collect();
        self
    }

    /// Inline loadSpec: the subcommand replaces the current level.
    pub(crate) fn apply_inline_spec(&mut self, sub: Rc<Subcommand>) {
        // ponytail: parserDirectives merge per object, not per field (4 inline specs upstream).
        if sub.has_directives() {
            self.directives = sub.directives().clone();
        }
        self.persistent = sub.persistent().clone();
        self.opt_args = ArgState::default();
        self.passed.clear();
        self.sub_args = ArgState::new(sub.args());
        self.entered_sub_args = false;
        self.set_completion(sub);
    }

    /// Merge a `generateSpec` result into the current level (SPEC §6.4).
    pub(crate) fn merge_generated_spec(&mut self, generated: &Subcommand) {
        let args_were_empty = self.completion.args().is_empty();
        let c = self.completion.merged_with_generated(generated);
        if args_were_empty {
            self.sub_args = ArgState::new(c.args());
        }
        self.persistent = generated
            .persistent()
            .iter()
            .chain(&self.persistent)
            .cloned()
            .collect();
        self.set_completion(Rc::new(c));
    }
}

/// First successful interpretation of `token`; None = it fits nowhere.
pub(crate) fn update(st: &ArgvState, token: &str, fin: bool) -> Option<ArgvState> {
    (st.can_subcommands()
        .then(|| subcommand(st, token, fin))
        .flatten())
    .or_else(|| {
        st.can_options()
            .then(|| option_or_chain(st, token, fin))
            .flatten()
    })
    .or_else(|| {
        st.prefer_opt_arg()
            .then(|| option_arg(st, token, fin))
            .flatten()
    })
    .or_else(|| subcommand_arg(st, token, fin))
}

/// The final token: its first interpretation, or a `None` annotation when it fits nowhere.
pub(crate) fn update_final(st: ArgvState, token: &str) -> ArgvState {
    update(&st, token, true).unwrap_or_else(|| {
        let mut st = st;
        st.annotations.push(ann(AnnotationKind::None, token));
        st
    })
}

fn subcommand(st: &ArgvState, token: &str, fin: bool) -> Option<ArgvState> {
    if st.entered_sub_args {
        return None;
    }
    let child = st
        .completion
        .subcommands()
        .get(*st.completion.sub_index().get(token)?)?
        .clone();
    let mut s = st.clone();
    s.annotations.push(ann(AnnotationKind::Subcommand, token));
    if fin {
        return Some(s); // siblings stay suggestable
    }
    if child.has_directives() {
        s.directives = child.directives().clone();
    }
    s.persistent.extend(child.persistent().iter().cloned());
    s.passed.clear();
    s.opt_args = ArgState::default();
    s.sub_args = ArgState::new(child.args());
    s.path.push(child.clone());
    s.completion = child;
    Some(s)
}

fn option(st: &ArgvState, flag: &str, fin: bool) -> Option<ArgvState> {
    let o = st.find_option(flag)?;
    if let Repeatability::Times(n) = o.is_repeatable() {
        let count = st
            .passed
            .iter()
            .filter(|p| p.names().iter().any(|x| o.names().contains(x)))
            .count();
        if count >= n as usize {
            return None;
        }
    }
    let mut s = st.clone();
    s.annotations.push(ann(AnnotationKind::Option, flag));
    if !fin {
        s.opt_args = ArgState::new(o.args());
        s.passed.push(o);
    }
    Some(s)
}

fn option_arg(st: &ArgvState, token: &str, fin: bool) -> Option<ArgvState> {
    st.opt_args.cur()?;
    let mut s = st.clone();
    s.annotations.push(ann(AnnotationKind::OptionArg, token));
    if !fin {
        s.opt_args.advance();
    }
    Some(s)
}

fn subcommand_arg(st: &ArgvState, token: &str, fin: bool) -> Option<ArgvState> {
    st.sub_args.cur()?;
    let mut s = st.clone();
    s.annotations.push(ann(AnnotationKind::SubcommandArg, token));
    if !fin {
        s.sub_args.advance();
        s.entered_sub_args = true;
    }
    Some(s)
}

fn option_or_chain(st: &ArgvState, token: &str, fin: bool) -> Option<ArgvState> {
    if fin && (token == "-" || token == "--") {
        return None;
    }
    if token == "--" {
        let mut s = st.clone();
        s.end_of_options = true;
        s.annotations.push(ann(AnnotationKind::Option, token));
        s.opt_args = ArgState::default();
        return Some(s);
    }
    let composite = |mut s: ArgvState, parts: Vec<(AnnotationKind, String)>| {
        s.annotations.truncate(st.annotations.len());
        let kind = parts.last().map_or(AnnotationKind::Option, |p| p.0);
        s.annotations.push(TokenAnnotation::new(kind, token.into(), parts));
        s
    };
    if st.directives.flags_posix_noncompliant() || token.starts_with("--") || !token.starts_with('-')
    {
        // Separators are matched char by char (a Set of strings vs. the token's chars).
        let seps = st.directives.option_arg_separators();
        if let Some((pos, c)) = token.char_indices().find(|&(_, c)| {
            seps.iter()
                .any(|s| s.len() == c.len_utf8() && s.starts_with(c))
        }) {
            let (flag, val) = (&token[..pos], &token[pos + c.len_utf8()..]);
            let s = option(st, flag, false)?;
            if s.opt_args.args.len() > 1 {
                return None;
            }
            let s = option_arg(&s, val, fin)?; // no arg slot → the whole token falls back to a positional
            return Some(composite(
                s,
                vec![
                    (AnnotationKind::Option, format!("{flag}{c}")),
                    (AnnotationKind::OptionArg, val.into()),
                ],
            ));
        }
        let mut s = option(st, token, fin)?;
        let o = st.find_option(token)?;
        if o.requires_equals() || o.requires_separator().is_some() {
            s.opt_args = ArgState::default(); // the next word is not its value
        }
        return Some(s);
    }
    // POSIX chain `-abc`.
    let chars: Vec<char> = token.chars().collect();
    let (mut cur, mut passed) = (st.clone(), st.passed.clone());
    let (mut opt_val, mut parts) = (String::new(), Vec::new());
    for i in 1..chars.len() {
        let flag = format!("-{}", chars[i]);
        passed = cur.passed.clone();
        match option(&cur, &flag, false) {
            Some(s) => cur = s,
            None if i > 1 => {
                opt_val = chars[i..].iter().collect();
                break;
            }
            None => return None,
        }
        parts.push((
            AnnotationKind::Option,
            if i == 1 { flag } else { chars[i].to_string() },
        ));
        if mandatory_or_var(cur.opt_args.cur()) {
            opt_val = chars[i + 1..].iter().collect();
            break;
        }
    }
    if !opt_val.is_empty() {
        if cur.opt_args.args.len() > 1 {
            return None;
        }
        cur = option_arg(&cur, &opt_val, fin)?;
        passed = cur.passed.clone();
        parts.push((AnnotationKind::OptionArg, opt_val));
    }
    if fin {
        cur.passed = passed; // the letter being typed stays suggestable
    }
    Some(composite(cur, parts))
}

pub(crate) fn result(st: ArgvState, last_text: &str, raw_last_token: &str, first_token: bool) -> ParsedCommandLine {
    let last = st.annotations.last();
    let mut search_term = last.map(|a| a.text().to_string()).unwrap_or_default();
    let mut only_args = st.end_of_options;
    let mut arg_st = st.arg_state();
    let mut option_chain = None;
    let mut raw = raw_last_token.to_string();
    if let Some(a) = last.filter(|a| !a.parts().is_empty()) {
        arg_st = &st.opt_args;
        match a.parts().last() {
            Some((AnnotationKind::OptionArg, t)) => {
                search_term = t.clone();
                only_args = true;
                // `--color=al` / `-oal`: only the value is replaced, never the flag in front of it
                let flag: String = a.parts()[..a.parts().len() - 1]
                    .iter()
                    .map(|p| p.1.as_str())
                    .collect();
                raw = raw_last_token
                    .strip_prefix(flag.as_str())
                    .unwrap_or(t)
                    .to_string();
            }
            Some((
                AnnotationKind::Subcommand
                | AnnotationKind::Option
                | AnnotationKind::SubcommandArg
                | AnnotationKind::None,
                _,
            ))
            | None => option_chain = Some(a.text().to_string()),
        }
    }
    let mut tokens = st.tokens.clone();
    if let Some(t) = tokens.last_mut() {
        *t = last_text.into();
    }
    ParsedCommandLine::new(ParsedCommandLineParts {
        current_arg: arg_st.cur().cloned(),
        search_term,
        suggest_subcommands: !only_args && st.can_subcommands(),
        suggest_options: !only_args && st.can_options(),
        suggest_args: true,
        completion: st.completion.clone(),
        path: st.path.clone(),
        passed_options: st.passed.clone(),
        persistent: st.persistent.clone(),
        command_index: st.command_index,
        tokens,
        raw_last_token: raw,
        directives: st.directives.clone(),
        option_chain,
        first_token,
        spec_name: st.spec_name.clone(),
        annotations: st.annotations,
    })
}
