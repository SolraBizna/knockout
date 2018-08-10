struct SlashFinder<'a> {
    cur: &'a [u8]
}

impl<'a> SlashFinder<'a> {
    pub fn new(src: &'a [u8]) -> SlashFinder<'a> {
        SlashFinder { cur: src }
    }
}

impl<'a> Iterator for SlashFinder<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        while !self.cur.is_empty() {
            let len = self.cur.len();
            if self.cur[len-1] == b'/' {
                self.cur = &self.cur[..len-1];
                return Some(len)
            }
            self.cur = &self.cur[..len-1];
        }
        None
    }
}

#[derive(Debug,Clone)]
enum Match {
    Literal(Vec<u8>),
    AnyByte,
    ZeroOrMoreNonSlash,
    ZeroOrMore,
    ByteInSet(Vec<(u8,u8)>),
    ByteNotInSet(Vec<(u8,u8)>),
    DirectoryOrAny,
}

#[derive(Debug,Clone)]
pub struct RsyncPattern {
    anchor_start: bool,
    req_dir: bool,
    full_path: bool,
    pattern: Vec<Match>,
    original: Vec<u8>,
}

fn byte_in_set(byte: u8, set: &[(u8,u8)]) -> bool {
    for (start, end) in set {
        if byte >= *start && byte <= *end { return true }
    }
    false
}

fn pattern_matches(mut rem: &[u8], mut pattern: &[Match], dir: bool) -> bool {
    loop {
        if let Some((el, rest)) = pattern.split_first() {
            pattern = rest;
            match el {
                Match::AnyByte => {
                    if rem.is_empty() { return false }
                    rem = &rem[1..];
                },
                Match::Literal(wat) => {
                    if !rem.starts_with(wat) { return false }
                    rem = &rem[wat.len()..];
                },
                Match::ByteInSet(set) => {
                    if rem.is_empty() { return false }
                    if !byte_in_set(rem[0], set) { return false }
                    rem = &rem[1..];
                },
                Match::ByteNotInSet(set) => {
                    if rem.is_empty() { return false }
                    if byte_in_set(rem[0], set) { return false }
                    rem = &rem[1..];
                },
                Match::ZeroOrMore => {
                    if pattern.is_empty() { return true }
                    loop {
                        if pattern_matches(rem, pattern, dir) { return true }
                        if rem.is_empty() { break }
                        else { rem = &rem[1..]; }
                    }
                    return false
                },
                Match::ZeroOrMoreNonSlash => {
                    if pattern.is_empty() { return !rem.contains(&b'/') }
                    loop {
                        if pattern_matches(rem, pattern, dir) { return true }
                        if rem.is_empty() { break }
                        else if rem[0] == b'/' { return false }
                        else { rem = &rem[1..]; }
                    }
                    return false
                },
                Match::DirectoryOrAny => {
                    return rem.starts_with(b"/")
                        || (rem.is_empty() && dir)
                },
            }
        }
        else {
            return rem.is_empty()
        }
    }
}

impl RsyncPattern {
    pub fn new(mut src: &[u8]) -> Result<RsyncPattern, &str> {
        let original = src.to_vec();
        let full_path = src.contains(&b'/');
        let anchor_start = if src.starts_with(b"/") {
            src = &src[1..];
            true
        } else { false };
        let req_dir = if src.ends_with(b"/") {
            src = &src[..src.len()-1];
            true
        } else { false };
        debug_assert!(!(anchor_start && !full_path));
        let mut pattern;
        if src.iter().filter(|x| **x == b'*' || **x == b'?' || **x == b'[')
        .next().is_some() {
            let mut rem = src;
            pattern = Vec::new();
            let mut literals = Vec::new();
            while !rem.is_empty() {
                let b = rem[0];
                rem = &rem[1..];
                if b == b'\\' {
                    if rem.len() < 1 {
                        return Err("Pattern contains a trailing `\\`")
                    }
                    literals.push(rem[0]);
                    rem = &rem[1..];
                }
                else if b == b'[' {
                    if !literals.is_empty() {
                        pattern.push(Match::Literal(literals));
                        literals = Vec::new();
                    }
                    let is_inverted;
                    if rem.starts_with(b"^") {
                        is_inverted = true;
                        rem = &rem[1..];
                    }
                    else {
                        is_inverted = false;
                    }
                    let mut set = Vec::new();
                    while set.is_empty() || !rem.starts_with(b"]") {
                        if rem.starts_with(b"[:") {
                            return Err("Character classes like [:alpha:] are \
                                        not supported")
                        }
                        if rem.is_empty() {
                            return Err("Pattern contains a character set with \
                                        no closing `]`")
                        }
                        let sch = rem[0];
                        let ech;
                        if rem.len() > 1 && rem[1] == b'-' {
                            if rem.len() < 3 {
                                return Err("Pattern contains a character set \
                                            with no closing `]`")
                            }
                            ech = rem[2];
                            rem = &rem[3..];
                        }
                        else {
                            ech = sch;
                            rem = &rem[1..];
                        }
                        if sch >= 0x80 || ech >= 0x80 {
                            return Err("Non-ASCII characters in character \
                                        sets (`[...]`) are not supported; \
                                        they most likely would not do what \
                                        you want anyway.")
                        }
                        set.push((sch, ech))
                    }
                    debug_assert!(rem.starts_with(b"]"));
                    rem = &rem[1..];
                    if is_inverted {
                        pattern.push(Match::ByteNotInSet(set))
                    }
                    else {
                        pattern.push(Match::ByteInSet(set))
                    }
                }
                else if b == b'?' {
                    if !literals.is_empty() {
                        pattern.push(Match::Literal(literals));
                        literals = Vec::new();
                    }
                    pattern.push(Match::AnyByte)
                }
                else if b == b'*' {
                    if !literals.is_empty() {
                        pattern.push(Match::Literal(literals));
                        literals = Vec::new();
                    }
                    if rem.starts_with(b"**") {
                        return Err("`***` is not supported in this location")
                    }
                    else if rem.starts_with(b"*") {
                        pattern.push(Match::ZeroOrMore);
                        rem = &rem[1..];
                    }
                    else {
                        pattern.push(Match::ZeroOrMoreNonSlash);
                    }
                }
                else if b == b'/' && rem == b"***" {
                    if !literals.is_empty() {
                        pattern.push(Match::Literal(literals));
                        literals = Vec::new();
                    }
                    pattern.push(Match::DirectoryOrAny);
                    rem = &[];
                }
                else {
                    literals.push(b);
                }
            }
            if !literals.is_empty() {
                pattern.push(Match::Literal(literals));
            }
        }
        else {
            pattern = vec![Match::Literal(src.to_vec())];
        }
        Ok(RsyncPattern {
            anchor_start, req_dir, full_path, pattern, original
        })
    }
    pub fn matches(&self, mut src: &[u8]) -> bool {
        debug_assert!(!src.starts_with(b"/"));
        let is_dir = src.ends_with(b"/");
        if is_dir {
            src = &src[..src.len()-1];
        }
        else if self.req_dir { return false }
        if self.full_path {
            if !self.anchor_start {
                for pos in SlashFinder::new(src) {
                    if pattern_matches(&src[pos..], &self.pattern, is_dir) {
                        return true
                    }
                }
            }
            return pattern_matches(src, &self.pattern, is_dir)
        }
        else {
            debug_assert!(!self.anchor_start);
            match SlashFinder::new(src).next() {
                Some(pos) => pattern_matches(&src[pos..], &self.pattern,
                                             is_dir),
                None => pattern_matches(src, &self.pattern, is_dir),
            }
        }
    }
    pub fn get_original_form(&self) -> &[u8] {
        &self.original
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    pub fn test() {
        let cases: &[(&[u8], &[&[u8]], &[&[u8]])] = &[
            // Match single-byte filenames
            (b"?", &[
                b"f",
                b"\x96",
            ], &[
                b"fo",
                b"foo",
                b"fooo",
                "井".as_bytes()
            ]),
            // Match two-byte filenames
            (b"??", &[
                b"fo",
            ], &[
                b"f",
                b"\x96",
                b"foo",
                b"fooo",
                "井".as_bytes()
            ]),
            // Match three-byte filenames
            (b"???", &[
                b"foo",
                "井".as_bytes()
            ], &[
                b"f",
                b"\x96",
                b"fo",
                b"fooo",
            ]),
            // Match four-byte filenames
            (b"????", &[
                b"fooo",
            ], &[
                b"f",
                b"\x96",
                b"fo",
                b"foo",
                "井".as_bytes()
            ]),
            // Test character class
            (b"foo_[ad-f]_version", &[
                b"foo_a_version",
                b"foo_d_version",
                b"foo_e_version",
                b"foo_f_version",
            ], &[
                b"foo_b_version",
                b"foo_c_version",
                b"foo_g_version",
            ]),
            // Simple filename match
            (b"foo", &[
                b"foo",
                b"dir/foo",
                b"cage/dir/foo",
            ], &[
                b"bar",
                b"foo/bar",
            ]),
            // Match a file in a directory
            (b"foo/bar", &[
                b"foo/bar",
                b"dir/foo/bar",
                b"cage/dir/foo/bar",
            ], &[
                b"bar",
                b"foo/bar/baz",
                b"cage/foo/bar/baz",
            ]),
            // Match only at the root
            (b"/foo/bar", &[
                b"foo/bar",
            ], &[
                b"dir/foo/bar",
                b"cage/dir/foo/bar",
                b"bar",
                b"foo/bar/baz",
                b"cage/foo/bar/baz",
            ]),
            // Match anything underneath a directory, including itself
            (b"foo/***", &[
                b"foo/",
                b"foo/bar",
                b"foo/bar/woof",
                b"foo/baz",
                b"foo/bang",
                b"cage/foo/",
                b"cage/foo/bar",
                b"cage/foo/bar/woof",
                b"cage/foo/baz",
                b"cage/foo/bang",
            ], &[
                b"foo",
                b"cage/foo",
            ]),
            // Match anything underneath a directory
            (b"foo/**", &[
                b"foo/bar",
                b"foo/bar/woof",
                b"foo/baz",
                b"foo/bang",
                b"cage/foo/bar",
                b"cage/foo/bar/woof",
                b"cage/foo/baz",
                b"cage/foo/bang",
            ], &[
                b"foo",
                b"foo/",
                b"cage/foo",
                b"cage/foo/",
            ]),
            // Match anything *directly* underneath an anchored directory
            (b"/foo/*", &[
                b"foo/bar",
                b"foo/baz",
                b"foo/bang",
            ], &[
                b"foo",
                b"foo/",
                b"foo/bar/woof",
                b"cage/foo",
                b"cage/foo/",
                b"cage/foo/bar",
                b"cage/foo/bar/woof",
                b"cage/foo/baz",
                b"cage/foo/bang",
            ]),
            // Match anything underneath a directory, anchored
            (b"/foo/**", &[
                b"foo/bar",
                b"foo/bar/woof",
                b"foo/baz",
                b"foo/bang",
            ], &[
                b"foo",
                b"foo/",
                b"cage/foo",
                b"cage/foo/",
                b"cage/foo/bar",
                b"cage/foo/bar/woof",
                b"cage/foo/baz",
                b"cage/foo/bang",
            ]),
            // Match anything *directly* underneath a directory, anchored
            (b"/foo/*", &[
                b"foo/bar",
                b"foo/baz",
                b"foo/bang",
            ], &[
                b"foo",
                b"foo/",
                b"foo/bar/woof",
                b"cage/foo",
                b"cage/foo/",
                b"cage/foo/bar",
                b"cage/foo/bar/woof",
                b"cage/foo/baz",
                b"cage/foo/bang",
            ]),
        ];
        let mut wrong = 0;
        for (source, matches, unmatches) in cases {
            let pattern = RsyncPattern::new(source).unwrap();
            for case in matches.iter() {
                if !pattern.matches(case) {
                    eprintln!("Expected pattern \"{}\" to match path \"{}\", \
                               but it did not\nPattern compiled to: {:?}",
                              String::from_utf8_lossy(source),
                              String::from_utf8_lossy(case),
                              pattern);
                    wrong += 1;
                }
            }
            for case in unmatches.iter() {
                if pattern.matches(case) {
                    eprintln!("Expected pattern \"{}\" not to match path \
                               \"{}\", but it did\nPattern compiled to: {:?}",
                              String::from_utf8_lossy(source),
                              String::from_utf8_lossy(case),
                              pattern);
                    wrong += 1;
                }
            }
        }
        if wrong > 0 { panic!("Some cases are wrong!") }
    }
}
