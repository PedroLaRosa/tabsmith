//! The JS half of the QuickJS spec runtime: `PRELUDE`, evaluated into every Context, and `wrap_module`, which turns a
//! spec module into a script for that Context.
//!
//! The protocol spans this file and the runtime, so keep every name. PRELUDE calls the host functions the runtime
//! registers on the Context's globals before it evaluates PRELUDE: `__ts_log(level, msg)`,
//! `__ts_exec(ctxId, inputJson) → opId`, `__ts_sleep(ms) → opId` and `__ts_done(task, ok, payload)`. The runtime
//! calls PRELUDE's entry points: `__ts.start(task, fnId, argsJson, ctxJson)` runs a spec callback and reports through
//! `__ts_done`, and `__ts.settle(opId, ok, payload)` resolves the host op whose resolver `__ts.op(opId)` registered.
//! `wrap_module`'s output calls `__ts.json`, which serialises a value with each function as `{"$fn": id}`.
//!
//! PRELUDE is evaluated exactly once per Context: its top-level `let`/`const` would throw on a second eval.

pub(crate) const PRELUDE: &str = r#"
globalThis.console = Object.fromEntries(["log","info","debug","warn","error"].map(l => [l, (...a) => __ts_log(l, a.map(String).join(" "))]));
let __tid = 0; const __live = new Set(); const __ops = new Map();
globalThis.setTimeout = (fn, ms = 0, ...args) => { const id = ++__tid; __live.add(id);
  __ts.op(__ts_sleep(+ms || 0)).then(() => { if (__live.delete(id)) fn(...args); }); return id; };
globalThis.clearTimeout = id => { __live.delete(id); };
globalThis.Intl ??= { NumberFormat: class { constructor(_l, o = {}) { this.o = o; }
  format(n) { if (this.o.notation !== "compact") return String(n);
    const u = [[1e12,"T"],[1e9,"B"],[1e6,"M"],[1e3,"K"]].find(([v]) => Math.abs(n) >= v);
    return u ? +(n / u[0]).toPrecision(this.o.maximumSignificantDigits ?? 3) + u[1] : String(n); } } };
globalThis.__ts = {
  fns: [], ids: new Map(),
  ser(v, seen = new WeakSet(), d = 0) {                 // functions → {"$fn": id}; drops undefined/symbol; cycle-safe
    if (typeof v === "function") { let id = this.ids.get(v);
      if (id === undefined) this.ids.set(v, id = this.fns.push(v) - 1); return { $fn: id }; }
    if (v === null || typeof v !== "object") return typeof v === "bigint" ? String(v) : v;
    if (v instanceof RegExp) return String(v);
    if (seen.has(v) || d > 200) return null; seen.add(v);
    const out = Array.isArray(v) ? v.map(x => this.ser(x, seen, d + 1))
      : Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined && typeof x !== "symbol")
                                            .map(([k, x]) => [k, this.ser(x, seen, d + 1)]));
    seen.delete(v); return out;                         // shared (non-cyclic) refs are duplicated, cycles → null
  },
  json(v) { return JSON.stringify(this.ser(v) ?? null); },
  op(id) { return new Promise((res, rej) => __ops.set(id, [res, rej])); },
  settle(id, ok, v) { const p = __ops.get(id); if (p) { __ops.delete(id); ok ? p[0](v) : p[1](new Error(v)); } },
  mkExec(ctx) {
    const run = input => __ts.op(__ts_exec(ctx.id, JSON.stringify(input))).then(JSON.parse);
    return input => typeof input === "string"
      ? run({ command: "sh", args: ["-c", input], cwd: ctx.cwd }).then(r => r.stdout)
      : run({ cwd: ctx.cwd, ...input });
  },
  call(id, argsJson, ctxJson) {                          // every spec callback goes through here
    const ctx = JSON.parse(ctxJson), args = JSON.parse(argsJson);
    for (const i of ctx.exec) args[i] = this.mkExec(ctx);
    return Promise.resolve().then(() => this.fns[id](...args)).then(r => this.json(r));
  },
  start(task, id, argsJson, ctxJson) {
    __ts.call(id, argsJson, ctxJson).then(r => __ts_done(task, true, r), e => __ts_done(task, false, String(e)));
  },
};
"#;

/// Rewrite an esbuild ES module (`…;export{a as default,b as versions};`, no imports — verified for every file of
/// the bundle) into a script returning `__ts.json` of its exports. A hand-written module (`export default spec;`, the
/// Fig docs form, common in dev/user/local specs) is accepted too.
pub(crate) fn wrap_module(src: &str) -> Option<String> {
    let Some((at, rest)) = src
        .rmatch_indices("export")
        .map(|(i, _)| (i, src[i + 6..].trim_start()))
        .find(|(_, rest)| rest.starts_with('{'))
    else {
        // ponytail: only `default` is exported here; named exports (`versions`) come from esbuild bundles anyway.
        let i = src.rfind("export default")?;
        let body: Vec<&str> = src[..i]
            .lines()
            .map(|l| l.strip_prefix("export ").unwrap_or(l))
            .collect();
        return Some(format!(
            "__ts.json((function(){{{}\nvar __ts_default={}\n;return {{default:__ts_default}};}})())",
            body.join("\n"),
            &src[i + 14..]
        ));
    };
    let clause = rest[1..].split('}').next()?;
    let fields: Vec<String> = clause
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once(" as ") {
            Some((local, exported)) => format!("{}:{}", exported.trim(), local.trim()),
            None => format!("{p}:{p}"),
        })
        .collect();
    Some(format!(
        "__ts.json((function(){{{}\n;return {{{}}};}})())",
        &src[..at],
        fields.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_module_rewrites_export_clause() {
        let s = wrap_module("var t=1,a=2;export{t as default,a as versions, b};").unwrap();
        assert_eq!(
            s,
            "__ts.json((function(){var t=1,a=2;\n;return {default:t,versions:a,b:b};})())"
        );
        assert!(wrap_module("var x = 1;").is_none());
        // `export` inside the body is not mistaken for the clause.
        assert!(wrap_module("var e=\"export\";export {e as default};\n")
            .unwrap()
            .ends_with("return {default:e};})())"));
    }
}
