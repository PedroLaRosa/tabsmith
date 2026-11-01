use crate::completion::domain::value_objects::insertion::Insertion;

/// Bytes for an insertion, reconciled with what the shell shows now (SPEC §7.3).
/// `ib` = buffer the reply was computed from, `cur` = buffer extracted now.
pub(crate) fn insertion_bytes(ins: &Insertion, ib: &str, cur: &str) -> Vec<u8> {
    let mut b = Vec::new();
    if let Some(extra) = cur.strip_prefix(ib) {
        b.extend(std::iter::repeat_n(0x7f, extra.chars().count())); // the user typed more meanwhile
    } else if let Some(missing) = ib.strip_prefix(cur) {
        b.extend_from_slice(missing.as_bytes()); // the shell has not echoed yet
    }
    b.extend(std::iter::repeat_n(0x7f, ins.delete()));
    b.extend_from_slice(ins.text().as_bytes());
    for _ in 0..ins.cursor_back() {
        b.extend_from_slice(b"\x1b[D");
    }
    if ins.executes() {
        b.push(b'\r');
    }
    b
}

/// Buffer expected once the shell applied an insertion (insertion-lock release, SPEC §7.3).
/// `cursor` = byte offset into `ib` the reply was computed at.
pub(crate) fn expected_buffer(ins: &Insertion, ib: &str, cursor: usize) -> String {
    let cursor = if ib.is_char_boundary(cursor) {
        cursor
    } else {
        ib.len()
    };
    let mut s = ib[..cursor].to_string();
    for _ in 0..ins.delete() {
        s.pop();
    }
    s.push_str(ins.text());
    s.push_str(&ib[cursor..]);
    s
}

/// Keys typed ahead that the extracted buffer `new` does not show yet. `base` = the buffer they were typed after
/// (the pending insertion's result while locked); `in_flight` = that insertion is not echoed yet.
pub(crate) fn unechoed_after(
    unechoed: &str,
    base: Option<&str>,
    new: Option<&str>,
    in_flight: bool,
) -> String {
    match (base, new) {
        (Some(b), Some(n)) => match n.strip_prefix(b) {
            Some(added) => unechoed.strip_prefix(added).unwrap_or("").to_string(),
            None if in_flight => unechoed.to_string(),
            None => String::new(),
        },
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::value_objects::insertion::InsertionParts;

    fn ins(delete: usize, text: &str, cursor_back: usize, execute: bool) -> Insertion {
        Insertion::new(InsertionParts {
            delete,
            text: text.into(),
            cursor_back,
            execute,
        })
    }

    #[test]
    fn insertion_bytes_reconcile() {
        let i = ins(0, "atus ", 0, false);
        assert_eq!(insertion_bytes(&i, "git st", "git st"), b"atus ");
        // Shell ahead of the reply (user typed `a` meanwhile): erase the extra chars first.
        assert_eq!(insertion_bytes(&i, "git st", "git sta"), b"\x7fatus ");
        assert_eq!(insertion_bytes(&i, "git st", "git stéé"), b"\x7f\x7fatus ");
        // Shell behind (echo pending): retype the missing tail first.
        assert_eq!(insertion_bytes(&i, "git st", "git s"), b"tatus ");
        // Diverged: no reconciliation.
        assert_eq!(insertion_bytes(&i, "git st", "ls"), b"atus ");
        // Delete, cursor-back and execute.
        let i = ins(3, "--out=", 1, true);
        assert_eq!(
            insertion_bytes(&i, "x --o", "x --o"),
            b"\x7f\x7f\x7f--out=\x1b[D\r"
        );
    }

    #[test]
    fn expected_buffer_after_insertion() {
        assert_eq!(
            expected_buffer(&ins(0, "atus ", 0, false), "git st", 6),
            "git status "
        );
        assert_eq!(
            expected_buffer(&ins(6, "my\\ dir/", 0, false), "cd my\\ di", 9),
            "cd my\\ dir/"
        );
        assert_eq!(
            expected_buffer(&ins(2, "checkout ", 0, false), "git ch | x", 6),
            "git checkout  | x"
        );
        assert_eq!(expected_buffer(&ins(1, "é", 0, false), "aé", 3), "aé");
    }

    #[test]
    fn unechoed_keys_are_consumed_by_their_echo() {
        assert_eq!(unechoed_after("lo", Some("hel"), Some("hel"), false), "lo");
        assert_eq!(unechoed_after("lo", Some("hel"), Some("hell"), false), "o");
        assert_eq!(unechoed_after("lo", Some("hel"), Some("hello"), false), "");
        assert_eq!(
            unechoed_after("lo", Some("hel"), Some("he"), false),
            "",
            "the shell did something else"
        );
        // Locked: typed after a pending insertion (base = its expected buffer).
        assert_eq!(
            unechoed_after("y", Some("cat abcd"), Some("cat ab"), true),
            "y"
        );
        assert_eq!(
            unechoed_after("y", Some("cat abcd"), Some("cat abcdy"), true),
            ""
        );
        assert_eq!(unechoed_after("y", None, Some("x"), false), "");
    }
}
