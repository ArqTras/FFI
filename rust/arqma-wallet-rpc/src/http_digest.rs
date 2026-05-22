//! MD5 digest auth (qop) — same contract as upstream `arqma-wallet-rpc` over HTTP.

use rand::Rng;
use std::collections::HashMap;

fn md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", md5::compute(bytes))
}

/// Random `cnonce` for digest (aligned with legacy JS wallet).
pub fn generate_cnonce() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill(&mut b);
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b);
    format!("{:x}", md5::compute(b64.as_bytes()))
}

/// Parse `WWW-Authenticate: Digest …` challenge fields.
pub fn parse_challenge(header: &str) -> HashMap<String, String> {
    let mut p = HashMap::new();
    if let Some(i) = header.find("Digest") {
        let rest = &header[i + "Digest".len()..];
        for part in rest.split(',') {
            if let Some(caps) = regex_simple(part) {
                p.insert(caps.0, caps.1);
            }
        }
    }
    p
}

fn regex_simple(part: &str) -> Option<(String, String)> {
    let t = part.trim();
    if let (Some(eq), Some(quote1), Some(quote2)) = (t.find('='), t.find('\"'), t.rfind('\"')) {
        if quote2 > quote1 {
            let k = t[..eq].trim().to_string();
            let v = t[quote1 + 1..quote2].to_string();
            return Some((k, v));
        }
    }
    None
}

fn first_qop(qop: &str) -> String {
    qop.split(',')
        .next()
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn response_hash(
    method: &str,
    path: &str,
    ch: &HashMap<String, String>,
    user: &str,
    pass: &str,
    nc: &str,
    cnonce: &str,
) -> Result<String, String> {
    let realm = ch.get("realm").ok_or("realm")?;
    let nonce = ch.get("nonce").ok_or("nonce")?;
    let ha1s = format!("{user}:{realm}:{pass}");
    let ha1_hex = md5_hex(ha1s.as_bytes());
    let ha2s = format!("{method}:{path}");
    let ha2_hex = md5_hex(ha2s.as_bytes());
    if let Some(qop) = ch.get("qop") {
        let q = first_qop(qop);
        if nc.is_empty() || cnonce.is_empty() {
            return Err("nc/cnonce".into());
        }
        let s = format!("{ha1_hex}:{nonce}:{nc}:{cnonce}:{q}:{ha2_hex}");
        Ok(md5_hex(s.as_bytes()))
    } else {
        let s = format!("{ha1_hex}:{nonce}:{ha2_hex}");
        Ok(md5_hex(s.as_bytes()))
    }
}

/// Build `Authorization: Digest …` for the second request after `401`.
pub fn build_digest_header(
    method: &str,
    path: &str,
    www: &str,
    user: &str,
    pass: &str,
    nc: &str,
    cnonce: &str,
) -> Result<String, String> {
    let ch = parse_challenge(www);
    let res = response_hash(method, path, &ch, user, pass, nc, cnonce)?;
    let realm = ch.get("realm").cloned().ok_or("realm")?;
    let nonce = ch.get("nonce").cloned().ok_or("nonce")?;
    let qop = ch.get("qop");
    let mut parts: Vec<String> = vec![
        format!("username=\"{user}\""),
        format!("realm=\"{realm}\""),
        format!("nonce=\"{nonce}\""),
        format!("uri=\"{path}\""),
        format!("cnonce=\"{cnonce}\""),
        "algorithm=MD5".to_string(),
        format!("nc={nc}"),
        format!("response=\"{res}\""),
    ];
    if let Some(q) = qop {
        parts.push(format!("qop={}", first_qop(q)));
    }
    Ok(format!("Digest {}", parts.join(", ")))
}

/// Increment `nc` (8 hex digits), wraps like legacy wallet.
pub fn inc_nc(nc: &str) -> String {
    if nc == "ffffffff" {
        return "00000001".to_string();
    }
    let v = u32::from_str_radix(nc, 16).unwrap_or(0).saturating_add(1);
    format!("{:08x}", v)
}
