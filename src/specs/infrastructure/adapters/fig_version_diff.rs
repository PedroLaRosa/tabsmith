//! Diff-versioned Fig specs: a spec's `versions` diffs merged onto its base with JS spread semantics (SPEC §6.3).
use crate::specs::domain::services::spec_version_selector::versions_to_apply;
use serde_json::{Map, Value};

/// Apply every diff whose version is `<= target` (all when `target` is None), in semver order.
pub(crate) fn apply_versions(base: Value, diffs: &Map<String, Value>, target: Option<&str>) -> Value {
    versions_to_apply(diffs.keys().map(String::as_str), target)
        .into_iter()
        .fold(base, |base, version| merge_subcommand(&base, &diffs[version]))
}

fn merge_subcommand(base: &Value, diff: &Value) -> Value {
    if diff.is_null() {
        return base.clone();
    }
    let mut out = shallow_merge(base, diff);
    if truthy(diff.get("subcommands")) {
        out["subcommands"] = merge_named(
            base.get("subcommands"),
            diff.get("subcommands"),
            merge_subcommand,
        );
    }
    if truthy(diff.get("options")) {
        out["options"] = merge_named(base.get("options"), diff.get("options"), merge_option);
    }
    if truthy(diff.get("args")) {
        out["args"] = merge_args(base.get("args"), diff.get("args"));
    }
    out
}

fn merge_option(base: &Value, diff: &Value) -> Value {
    let mut out = shallow_merge(base, diff);
    if truthy(diff.get("args")) {
        out["args"] = merge_args(base.get("args"), diff.get("args"));
    }
    out
}

/// Diff items matched to base items by name overlap; `remove` drops the match. Order: diff items, then untouched base.
fn merge_named(
    base: Option<&Value>,
    diff: Option<&Value>,
    merge: fn(&Value, &Value) -> Value,
) -> Value {
    let (bl, dl) = (make_array(base), make_array(diff));
    let mut used = vec![false; bl.len()];
    let mut out = Vec::new();
    for d in &dl {
        let dn = names(d);
        match (0..bl.len()).find(|&j| !used[j] && names(&bl[j]).iter().any(|n| dn.contains(n))) {
            None => out.push(d.clone()),
            Some(j) => {
                used[j] = true;
                if d.get("remove").is_none() {
                    out.push(merge(&bl[j], d));
                }
            }
        }
    }
    out.extend(
        bl.iter()
            .zip(used)
            .filter(|(_, u)| !u)
            .map(|(b, _)| b.clone()),
    );
    Value::Array(out)
}

/// Positional merge; the index is taken after filtering out removed args (quirk kept).
fn merge_args(base: Option<&Value>, diff: Option<&Value>) -> Value {
    let bl = make_array(base);
    let dl = make_array(diff)
        .into_iter()
        .filter(|d| !truthy(d.get("remove")));
    Value::Array(
        dl.enumerate()
            .map(|(i, d)| shallow_merge(bl.get(i).unwrap_or(&Value::Null), &d))
            .collect(),
    )
}

/// `{...base, ...diff}`.
fn shallow_merge(base: &Value, diff: &Value) -> Value {
    let mut out = base.as_object().cloned().unwrap_or_default();
    for (k, v) in diff.as_object().into_iter().flatten() {
        out.insert(k.clone(), v.clone());
    }
    Value::Object(out)
}

fn make_array(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(a)) => a.clone(),
        Some(x) if truthy(Some(x)) => vec![x.clone()],
        _ => Vec::new(),
    }
}

fn names(v: &Value) -> Vec<Value> {
    make_array(v.get("name"))
}

/// JS truthiness.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diff_merge() {
        let base = json!({
            "name": "h", "description": "old",
            "subcommands": [{"name": "a", "description": "A"}, {"name": ["b", "bb"], "description": "B"}, {"name": "c"}],
            "options": [{"name": "--x", "args": [{"name": "x1"}, {"name": "x2"}]}, {"name": "--y"}],
            "args": [{"name": "p0", "description": "d0"}, {"name": "p1", "description": "d1"}]
        });
        let diff = json!({
            "description": "new",
            "subcommands": [{"name": "bb", "description": "B2"}, {"name": "new"}, {"name": "c", "remove": true}],
            "options": [{"name": "--x", "args": [{"name": "gone", "remove": true}, {"description": "second"}]}],
            "args": [{"remove": true}, {"description": "z"}]
        });
        let out = merge_subcommand(&base, &diff);
        assert_eq!(out["description"], "new");
        // diff items first (merged, new), then untouched base items; `remove` drops the match
        assert_eq!(
            out["subcommands"],
            json!([{"name": "bb", "description": "B2"}, {"name": "new"}, {"name": "a", "description": "A"}])
        );
        // merge_args indexes after filtering: "second" lands on x1, "z" on p0 (quirk kept)
        assert_eq!(
            out["options"],
            json!([{"name": "--x", "args": [{"name": "x1", "description": "second"}]}, {"name": "--y"}])
        );
        assert_eq!(out["args"], json!([{"name": "p0", "description": "z"}]));
        assert_eq!(merge_subcommand(&base, &Value::Null), base);
        assert_eq!(
            merge_subcommand(&base, &json!({"subcommands": []}))["subcommands"],
            base["subcommands"]
        );
    }

    #[test]
    fn version_pick() {
        let base = json!({"name": "h", "description": "base"});
        let diffs = json!({
            "9.0.0": {"description": "nine"},
            "8.10.0": {"description": "eight-ten"},
            "8.2.0": {"description": "eight-two", "subcommands": [{"name": "x"}]}
        });
        let diffs = diffs.as_object().unwrap();
        let desc = |t: Option<&str>| {
            apply_versions(base.clone(), diffs, t)["description"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(desc(Some("8.10.0")), "eight-ten"); // semver order, not string order
        assert_eq!(desc(Some("8.5.0")), "eight-two");
        assert_eq!(desc(None), "nine");
        assert_eq!(desc(Some("1.0.0")), "base");
        assert_eq!(desc(Some("garbage")), "nine");
        assert_eq!(
            apply_versions(base.clone(), diffs, Some("8.10.0"))["subcommands"],
            json!([{"name": "x"}])
        );
    }
}
