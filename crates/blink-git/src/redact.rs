use std::ops::Range;

const MASK: &str = "***";
const TOKEN_PREFIXES: [&str; 6] = ["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"];
const MIN_TOKEN_BODY: usize = 16;

pub fn redact(text: &str) -> String {
    let text = redact_userinfo(text);
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(secret) = next_secret(&lower, cursor) {
        out.push_str(&text[cursor..secret.start]);
        out.push_str(MASK);
        cursor = secret.end;
    }
    out.push_str(&text[cursor..]);
    out
}

fn redact_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let authority_len = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '\'' | '"' | '`' | '<' | '>'))
            .unwrap_or(tail.len());
        let authority = &tail[..authority_len];
        match authority.rfind('@') {
            Some(at) => {
                out.push_str(MASK);
                out.push_str(&authority[at..]);
            }
            None => out.push_str(authority),
        }
        rest = &tail[authority_len..];
    }
    out.push_str(rest);
    out
}

fn next_secret(lower: &str, from: usize) -> Option<Range<usize>> {
    [authorization_value(lower, from), bearer_value(lower, from), github_token(lower, from)]
        .into_iter()
        .flatten()
        .min_by_key(|r| r.start)
}

fn authorization_value(lower: &str, from: usize) -> Option<Range<usize>> {
    lower[from..].match_indices("authorization").find_map(|(i, needle)| {
        let rest = lower[from + i + needle.len()..].trim_start_matches([' ', '\t']);
        let value = rest.strip_prefix([':', '='])?.trim_start_matches([' ', '\t']);
        let start = lower.len() - value.len();
        let len = value.find(['\r', '\n', '"', '\'']).unwrap_or(value.len());
        (len > 0).then(|| start..start + len)
    })
}

fn bearer_value(lower: &str, from: usize) -> Option<Range<usize>> {
    lower[from..].match_indices("bearer").find_map(|(i, needle)| {
        let at = from + i;
        if !at_word_start(lower, at) {
            return None;
        }
        let rest = &lower[at + needle.len()..];
        let value = rest.trim_start_matches([' ', '\t']);
        if value.len() == rest.len() {
            return None;
        }
        let start = lower.len() - value.len();
        let len = value.find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';')).unwrap_or(value.len());
        (len > 0).then(|| start..start + len)
    })
}

fn github_token(lower: &str, from: usize) -> Option<Range<usize>> {
    TOKEN_PREFIXES
        .iter()
        .filter_map(|prefix| {
            lower[from..].match_indices(prefix).find_map(|(i, _)| {
                let at = from + i;
                let start = at + prefix.len();
                let len = lower[start..].bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
                (at_word_start(lower, at) && len >= MIN_TOKEN_BODY).then(|| start..start + len)
            })
        })
        .min_by_key(|r| r.start)
}

fn at_word_start(lower: &str, at: usize) -> bool {
    lower[..at].bytes().next_back().is_none_or(|b| !b.is_ascii_alphanumeric() && b != b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_url_userinfo() {
        assert_eq!(
            redact("fatal: unable to access 'https://alice:hunter2@github.com/acme/app.git/': 403"),
            "fatal: unable to access 'https://***@github.com/acme/app.git/': 403"
        );
        assert_eq!(
            redact("https://x-access-token:v1.abcdef0123456789@github.com/acme/app"),
            "https://***@github.com/acme/app"
        );
        assert_eq!(redact("http://token@example.com"), "http://***@example.com");
        assert_eq!(redact("https://github.com/acme/app"), "https://github.com/acme/app");
        assert_eq!(redact("git@github.com:acme/app.git"), "git@github.com:acme/app.git");
    }

    #[test]
    fn redacts_authorization_headers() {
        assert_eq!(
            redact("> Authorization: Basic eDp5eg==\n> Host: github.com"),
            "> Authorization: ***\n> Host: github.com"
        );
        assert_eq!(redact("http.extraheader=AUTHORIZATION: bearer abc.def"), "http.extraheader=AUTHORIZATION: ***");
        assert_eq!(redact("sent Bearer abc123.xyz to api"), "sent Bearer *** to api");
        assert_eq!(redact(r#"header="Authorization: token abc" next"#), r#"header="Authorization: ***" next"#);
        assert_eq!(redact("authorization failed"), "authorization failed");
        assert_eq!(redact("forebearer stays"), "forebearer stays");
    }

    #[test]
    fn redacts_github_token_shapes() {
        let body = "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
        for prefix in TOKEN_PREFIXES {
            let token = format!("{prefix}{body}");
            assert_eq!(redact(&format!("token={token} end")), format!("token={prefix}*** end"));
        }
        assert_eq!(redact("branch ghp_short"), "branch ghp_short");
        assert_eq!(redact(&format!("xghp_{body}")), format!("xghp_{body}"));
    }

    #[test]
    fn handles_multiple_secrets_and_unicode() {
        let line = "café https://u:p@h.io/x ghs_0123456789abcdefghij Bearer zz";
        assert_eq!(redact(line), "café https://***@h.io/x ghs_*** Bearer ***");
    }
}
