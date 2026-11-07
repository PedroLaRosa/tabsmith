/// fuzzysort `single(q, t)` (fresh struct per call): `(score, matched char indexes)`; higher is better, 0 = exact.
pub(crate) fn fuzzysort(q: &str, t: &str) -> Option<(i64, Vec<usize>)> {
    if q.is_empty() || t.is_empty() {
        return None;
    }
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let ql: Vec<char> = q.chars().map(lower).collect();
    let tc: Vec<char> = t.chars().collect();
    let tl: Vec<char> = tc.iter().map(|&c| lower(c)).collect();
    let (qn, tn) = (ql.len(), tl.len());
    // simple pass: query chars in order
    let mut simple = Vec::with_capacity(qn);
    for (ti, &c) in tl.iter().enumerate() {
        if c == ql[simple.len()] {
            simple.push(ti);
            if simple.len() == qn {
                break;
            }
        }
    }
    if simple.len() < qn {
        return None;
    }
    // beginnings: upper after non-upper, first alnum after non-alnum, any non-alnum (ASCII, original case)
    let mut beginning = vec![false; tn];
    let (mut was_upper, mut was_alnum) = (false, false);
    for (i, &c) in tc.iter().enumerate() {
        let (upper, alnum) = (c.is_ascii_uppercase(), c.is_ascii_alphanumeric());
        beginning[i] = (upper && !was_upper) || !was_alnum || !alnum;
        was_upper = upper;
        was_alnum = alnum;
    }
    let mut next = vec![tn; tn];
    let mut nb = tn;
    for i in (0..tn).rev() {
        next[i] = nb;
        if beginning[i] {
            nb = i;
        }
    }
    // strict pass: only consecutive matches or word beginnings, with backtracking
    let mut strict: Vec<usize> = Vec::with_capacity(qn);
    let mut ok = false;
    let mut ti = if simple[0] == 0 {
        0
    } else {
        next[simple[0] - 1]
    };
    if ti != tn {
        let mut qi = 0;
        loop {
            if ti >= tn {
                if qi == 0 {
                    break;
                }
                qi -= 1;
                let last = strict.pop().unwrap_or(0);
                ti = next[last];
            } else if ql[qi] == tl[ti] {
                strict.push(ti);
                qi += 1;
                if qi == qn {
                    ok = true;
                    break;
                }
                ti += 1;
            } else {
                ti = next[ti];
            }
        }
    }
    let best = if ok { strict } else { simple };
    let mut score: i64 = 0;
    let mut last: i64 = -1;
    for &i in &best {
        let i = i as i64;
        if last != i - 1 {
            score -= i;
        }
        last = i;
    }
    if !ok {
        score *= 1000;
    }
    score -= (tn - qn) as i64;
    Some((score, best))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzysort_vectors() {
        assert_eq!(fuzzysort("fs", "Fuzzy Search"), Some((-16, vec![0, 6])));
        assert_eq!(fuzzysort("test", "test").map(|r| r.0), Some(0));
        assert_eq!(fuzzysort("doesnt exist", "target"), None);
        assert_eq!(fuzzysort("", "x"), None);
        // no strict match (not at a word beginning) → simple indexes, ×1000
        assert_eq!(fuzzysort("ut", "output"), Some((-1004, vec![1, 2])));
    }
}
