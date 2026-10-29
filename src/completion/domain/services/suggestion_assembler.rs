use crate::completion::domain::services::suggestion_filter::cmp_locale;
use crate::completion::domain::value_objects::generator_slot_view::GeneratorSlotView;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::cmp::Ordering;
use std::collections::HashSet;

fn cmp_first(a: &SuggestionCandidate, b: &SuggestionCandidate) -> Ordering {
    cmp_locale(a.first(), b.first())
}

/// Options offered at this level: exclusiveOn / repeat limits / dependsOn priority, alphabetical.
fn static_options(pr: &ParsedCommandLine) -> Vec<SuggestionCandidate> {
    let passed = pr.passed_options();
    let excluded: HashSet<&str> = passed
        .iter()
        .flat_map(|o| o.exclusive_on().iter().map(String::as_str))
        .collect();
    let mut unmet: HashSet<&str> = passed
        .iter()
        .flat_map(|o| o.depends_on().iter().map(String::as_str))
        .collect();
    for o in passed {
        for n in o.names() {
            unmet.remove(n.as_str());
        }
    }
    let comp = pr.completion();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for o in comp
        .options()
        .iter()
        .chain(pr.persistent())
        .chain(comp.persistent())
    {
        // "unique": the same option may be reachable as local and inherited persistent
        if o.names().iter().any(|n| seen.contains(n.as_str())) {
            continue;
        }
        seen.extend(o.names().iter().map(String::as_str));
        if o.names().iter().any(|n| excluded.contains(n.as_str())) {
            continue;
        }
        let limit = match o.is_repeatable() {
            Repeatability::No | Repeatability::Times(0) => Some(1),
            Repeatability::Times(n) => Some(n as usize),
            Repeatability::Unlimited => None,
        };
        if let Some(limit) = limit {
            let used = passed
                .iter()
                .filter(|p| p.names().iter().any(|n| o.names().contains(n)))
                .count();
            if used >= limit {
                continue;
            }
        }
        let mut s = SuggestionCandidate::from_opt(o, pr.directives().option_arg_separators());
        if o.names().iter().any(|n| unmet.contains(n.as_str())) {
            s = s.with_priority(75.0);
        }
        out.push(s);
    }
    out.sort_by(cmp_first);
    out
}

/// `-x` short name of an option row.
fn short_name(s: &SuggestionCandidate) -> Option<String> {
    s.names()
        .iter()
        .find(|n| n.starts_with('-') && !n.starts_with("--") && n.chars().count() == 2)
        .cloned()
}

/// Build the unfiltered list for a parse result: subcommands, args (static + generators), additional
/// suggestions, options, then option-chain rewriting.
pub(crate) fn assemble(pr: &ParsedCommandLine, gens: &[GeneratorSlotView]) -> Vec<SuggestionCandidate> {
    let comp = pr.completion();
    let chain = pr.option_chain().as_deref();
    let mut out = Vec::new();
    if pr.suggest_subcommands() {
        let mut subs: Vec<SuggestionCandidate> = comp.subcommands().iter().map(|s| SuggestionCandidate::from_sub(s)).collect();
        subs.sort_by(cmp_first);
        out.extend(subs);
    }
    if pr.suggest_args() {
        if let Some(arg) = pr.current_arg() {
            out.extend(
                arg.suggestions()
                    .iter()
                    .map(|s| SuggestionCandidate::from_suggestion(s, arg.is_dangerous(), None, SuggestionQuery::Term)),
            );
            for g in gens {
                // SPEC §6.6.1: stale results of a loading generator only without getQueryTerm
                if !g.loading || g.query == SuggestionQuery::Term {
                    out.extend(
                        g.items
                            .iter()
                            .map(|s| SuggestionCandidate::from_suggestion(s, false, Some(g.key), g.query.clone())),
                    );
                }
            }
        }
    }
    let mut additional: Vec<SuggestionCandidate> = comp
        .additional_suggestions()
        .iter()
        // untyped rows are not `arg`s: dropped inside an option chain (only option/arg types are kept)
        .filter(|s| chain.is_none() || s.kind().is_some())
        .map(|s| SuggestionCandidate::from_suggestion(s, comp.is_dangerous(), None, SuggestionQuery::Term))
        .collect();
    additional.sort_by(cmp_first);
    out.extend(additional);
    let options = static_options(pr);
    if pr.suggest_options() {
        out.extend(options.iter().cloned());
    }
    match chain {
        Some(chain) => option_chain(out, &options, chain, pr.suggest_options()),
        None => out,
    }
}

/// SPEC §6.7.1 step 5 (typing `-la`).
fn option_chain(list: Vec<SuggestionCandidate>, options: &[SuggestionCandidate], chain: &str, options_flag: bool) -> Vec<SuggestionCandidate> {
    let Some(last) = chain.chars().last().map(|c| format!("-{c}")) else {
        return list;
    };
    let mut list: Vec<SuggestionCandidate> = list
        .into_iter()
        .filter(|s| matches!(s.kind(), SuggestionKind::Option | SuggestionKind::Arg))
        .collect();
    if !options_flag {
        if let Some(o) = options
            .iter()
            .find(|o| short_name(o).as_deref() == Some(last.as_str()))
        {
            list.push(o.clone());
        }
    }
    for s in &mut list {
        if s.kind() == SuggestionKind::Arg {
            s.set_query(SuggestionQuery::Value(String::new()));
            continue;
        }
        let Some(short) = short_name(s) else { continue };
        if short == last {
            let names = s
                .names()
                .iter()
                .map(|n| if *n == short { chain.to_string() } else { n.clone() })
                .collect();
            s.set_names(names);
        } else {
            let name = format!("{chain}{}", &short[1..]);
            s.set_names(vec![name.clone()]);
            s.set_insert_value(Some(name));
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::services::suggestion_ranker::rank;
    use crate::completion::domain::test_fixtures::{names, opt, opts, pr_for, sub};
    use crate::completion::domain::value_objects::annotation_kind::AnnotationKind;
    use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
    use crate::specs::domain::entities::subcommand::Subcommand;
    use std::rc::Rc;

    #[test]
    fn assembly_options_rules() {
        let excl = Rc::new(
            opt(&["-a"])
                .with_exclusive_on(vec!["-b".into()])
                .with_depends_on(vec!["-c".into()]),
        );
        let comp = Subcommand::default()
            .with_subcommands(vec![sub(&["zeta"]), sub(&["Alpha"]), sub(&["beta"])])
            .with_options(vec![
                excl.clone(),
                Rc::new(opt(&["-b"])),
                Rc::new(opt(&["-c"])),
                Rc::new(opt(&["-v"]).with_is_repeatable(Repeatability::Unlimited)),
            ]);
        let mut pr = pr_for(comp, "");
        pr.passed_options = vec![
            excl,
            Rc::new(opt(&["-v"]).with_is_repeatable(Repeatability::Unlimited)),
        ];
        let out = assemble(&ParsedCommandLine::new(pr), &[]);
        let got: Vec<(&str, f64)> = out.iter().map(|s| (s.first(), s.priority())).collect();
        // subcommands alphabetical; -a used once (gone), -b excluded, -c depended on → 75, -v repeatable
        assert_eq!(
            got,
            [
                ("Alpha", 0.0),
                ("beta", 0.0),
                ("zeta", 0.0),
                ("-c", 75.0),
                ("-v", 0.0)
            ]
        );
    }

    #[test]
    fn option_chain_rewriting() {
        let l = Rc::new(opt(&["-l"]));
        let comp = Subcommand::default()
            .with_options(vec![
                l.clone(),
                Rc::new(opt(&["-a", "--all"])),
                Rc::new(opt(&["-h"])),
                Rc::new(opt(&["--long"])),
            ])
            .with_subcommands(vec![sub(&["sub"])]);
        let mut pr = pr_for(comp, "-la");
        pr.passed_options = vec![l];
        pr.option_chain = Some("-la".into());
        pr.annotations = vec![TokenAnnotation::new(AnnotationKind::Option, "-la".into(), vec![])];
        let out = assemble(&ParsedCommandLine::new(pr), &[]);
        let got: Vec<Vec<String>> = out.iter().map(|s| s.names().clone()).collect();
        assert_eq!(
            got,
            [
                vec!["--long".to_string()],
                vec!["-la".into(), "--all".into()],
                vec!["-lah".into()]
            ]
        );
        assert_eq!(out[2].insert_value().as_deref(), Some("-lah"));
        assert_eq!(
            names(&rank(out, "-la", None, &opts())),
            ["!-la", "-la", "-lah"]
        );
    }
}
