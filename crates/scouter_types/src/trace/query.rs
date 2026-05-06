//! Lucene-style search-bar query parser for `TraceFilters`.
//!
//! Grammar:
//! ```text
//! expr     := or_expr
//! or_expr  := and_expr ( ( "OR" | "|" )  and_expr )*
//! and_expr := unary    ( ( "AND" | "&" | <implicit> ) unary )*
//! unary    := ( "NOT" | "-" )? atom
//! atom     := "(" expr ")" | quoted | term
//! term     := key ":" value | "duration" op value | bare_word
//! op       := ">=" | "<=" | ">" | "<"
//! quoted   := "\"" .*? "\"" | "'" .*? "'"
//! ```
//!
//! `>` and `>=`, and `<` and `<=`, both lower to inclusive millisecond bounds.

use crate::error::TypeError;
use crate::trace::sql::TraceFilters;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

const MAX_DEPTH: usize = 32;
const MAX_CLAUSE_NODES: usize = 512;
const MAX_CLAUSE_FANOUT: usize = 64;
const MAX_CLAUSE_LEAF_STRING_LEN: usize = MAX_QUERY_LEN;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(no_recursion))]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum FilterClause {
    And(Vec<FilterClause>),
    Or(Vec<FilterClause>),
    Not(Box<FilterClause>),
    Phrase(String),
    Service(String),
    ServiceNamespace(String),
    ServiceVersion(String),
    ServiceInstanceId(String),
    StatusCode(i32),
    HasErrors(bool),
    DurationMinMs(i64),
    DurationMaxMs(i64),
    Attr { key: String, value: String },
}

impl FilterClause {
    pub fn and_merge(a: Option<FilterClause>, b: Option<FilterClause>) -> Option<FilterClause> {
        match (a, b) {
            (None, None) => None,
            (Some(c), None) | (None, Some(c)) => Some(c),
            (Some(left), Some(right)) => Some(FilterClause::And(vec![left, right])),
        }
    }
}

const MAX_QUERY_LEN: usize = 4096;

pub fn validate_filter_clause(clause: &FilterClause) -> Result<(), TypeError> {
    let mut stats = ClauseValidationStats::default();
    validate_clause_node(clause, 0, &mut stats).map(|_| ())
}

#[derive(Default)]
struct ClauseValidationStats {
    nodes: usize,
}

#[derive(Default, Copy, Clone)]
struct DurationBounds {
    min: Option<i64>,
    max: Option<i64>,
}

impl DurationBounds {
    fn merge_and(&mut self, other: DurationBounds) {
        if let Some(min) = other.min {
            self.min = Some(self.min.map_or(min, |current| current.max(min)));
        }
        if let Some(max) = other.max {
            self.max = Some(self.max.map_or(max, |current| current.min(max)));
        }
    }

    fn validate(self) -> Result<(), TypeError> {
        if let (Some(lo), Some(hi)) = (self.min, self.max)
            && lo > hi
        {
            return Err(TypeError::ParseError(format!(
                "impossible duration range: min {lo}ms > max {hi}ms"
            )));
        }
        Ok(())
    }
}

fn validate_clause_node(
    clause: &FilterClause,
    depth: usize,
    stats: &mut ClauseValidationStats,
) -> Result<DurationBounds, TypeError> {
    if depth > MAX_DEPTH {
        return Err(TypeError::ParseError(format!(
            "filter clause nesting too deep (max {MAX_DEPTH})"
        )));
    }

    stats.nodes += 1;
    if stats.nodes > MAX_CLAUSE_NODES {
        return Err(TypeError::ParseError(format!(
            "filter clause has too many nodes: {} (max {MAX_CLAUSE_NODES})",
            stats.nodes
        )));
    }

    match clause {
        FilterClause::And(parts) => {
            validate_fanout(parts.len())?;

            let mut bounds = DurationBounds::default();
            for part in parts {
                bounds.merge_and(validate_clause_node(part, depth + 1, stats)?);
            }
            bounds.validate()?;
            Ok(bounds)
        }
        FilterClause::Or(parts) => {
            validate_fanout(parts.len())?;

            for part in parts {
                validate_clause_node(part, depth + 1, stats)?;
            }
            Ok(DurationBounds::default())
        }
        FilterClause::Not(inner) => {
            validate_clause_node(inner, depth + 1, stats)?;
            Ok(DurationBounds::default())
        }
        FilterClause::Phrase(value)
        | FilterClause::Service(value)
        | FilterClause::ServiceNamespace(value)
        | FilterClause::ServiceVersion(value)
        | FilterClause::ServiceInstanceId(value) => {
            validate_leaf_string(value)?;
            Ok(DurationBounds::default())
        }
        FilterClause::Attr { key, value } => {
            validate_leaf_string(key)?;
            validate_leaf_string(value)?;
            Ok(DurationBounds::default())
        }
        FilterClause::DurationMinMs(value) => {
            validate_duration_value(*value)?;
            Ok(DurationBounds {
                min: Some(*value),
                max: None,
            })
        }
        FilterClause::DurationMaxMs(value) => {
            validate_duration_value(*value)?;
            Ok(DurationBounds {
                min: None,
                max: Some(*value),
            })
        }
        FilterClause::StatusCode(_) | FilterClause::HasErrors(_) => Ok(DurationBounds::default()),
    }
}

fn validate_fanout(len: usize) -> Result<(), TypeError> {
    if len > MAX_CLAUSE_FANOUT {
        return Err(TypeError::ParseError(format!(
            "filter clause boolean fanout too large: {len} (max {MAX_CLAUSE_FANOUT})"
        )));
    }
    Ok(())
}

fn validate_leaf_string(value: &str) -> Result<(), TypeError> {
    if value.len() > MAX_CLAUSE_LEAF_STRING_LEN {
        return Err(TypeError::ParseError(format!(
            "filter clause string too long: {} chars (max {MAX_CLAUSE_LEAF_STRING_LEN})",
            value.len()
        )));
    }
    Ok(())
}

fn validate_duration_value(value: i64) -> Result<(), TypeError> {
    if value < 0 {
        return Err(TypeError::ParseError(format!(
            "duration must be >= 0: {value}ms"
        )));
    }
    Ok(())
}

pub fn parse_search_query(q: &str) -> Result<TraceFilters, TypeError> {
    let mut filters = TraceFilters::default();
    let trimmed = q.trim();
    if trimmed.is_empty() {
        return Ok(filters);
    }
    if trimmed.len() > MAX_QUERY_LEN {
        return Err(TypeError::ParseError(format!(
            "query too long: {} chars (max {MAX_QUERY_LEN})",
            trimmed.len()
        )));
    }

    let toks = tokenize(trimmed)?;
    let mut parser = Parser { toks, pos: 0 };
    filters.clause = parser.parse_or(&mut filters, 0)?;
    parser.expect_eof()?;
    Ok(filters)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Quoted(String),
    Word(String),
}

fn tokenize(q: &str) -> Result<Vec<Tok>, TypeError> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let bytes = q.as_bytes();
    let mut i = 0;
    let mut at_boundary = true;

    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            ' ' | '\t' | '\n' | '\r' | ',' => {
                push_word(&mut buf, &mut out);
                at_boundary = true;
                i += 1;
            }
            '(' => {
                push_word(&mut buf, &mut out);
                out.push(Tok::LParen);
                at_boundary = true;
                i += 1;
            }
            ')' => {
                push_word(&mut buf, &mut out);
                out.push(Tok::RParen);
                at_boundary = true;
                i += 1;
            }
            '"' | '\'' => {
                push_word(&mut buf, &mut out);
                let quote = c;
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < bytes.len() {
                    let qc = bytes[i] as char;
                    if qc == '\\' && i + 1 < bytes.len() {
                        s.push(bytes[i + 1] as char);
                        i += 2;
                    } else if qc == quote {
                        closed = true;
                        i += 1;
                        break;
                    } else {
                        s.push(qc);
                        i += 1;
                    }
                }
                if !closed {
                    return Err(TypeError::ParseError("unterminated quoted string".into()));
                }
                out.push(Tok::Quoted(s));
                at_boundary = false;
            }
            '&' if at_boundary && buf.is_empty() => {
                out.push(Tok::And);
                at_boundary = true;
                i += 1;
            }
            '|' if at_boundary && buf.is_empty() => {
                out.push(Tok::Or);
                at_boundary = true;
                i += 1;
            }
            '-' if at_boundary && buf.is_empty() => {
                if let Some(&next) = bytes.get(i + 1) {
                    let nc = next as char;
                    if !matches!(nc, ' ' | '\t' | '\n' | '\r' | '(' | ')' | '&' | '|' | '-') {
                        out.push(Tok::Not);
                        at_boundary = true;
                        i += 1;
                        continue;
                    }
                }
                buf.push(c);
                at_boundary = false;
                i += 1;
            }
            _ => {
                buf.push(c);
                at_boundary = false;
                i += 1;
            }
        }
    }
    push_word(&mut buf, &mut out);
    Ok(out)
}

fn push_word(buf: &mut String, out: &mut Vec<Tok>) {
    if buf.is_empty() {
        return;
    }
    let s = std::mem::take(buf);
    match s.as_str() {
        "AND" => out.push(Tok::And),
        "OR" => out.push(Tok::Or),
        "NOT" => out.push(Tok::Not),
        _ => out.push(Tok::Word(s)),
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let tok = self.toks.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect_eof(&self) -> Result<(), TypeError> {
        if self.pos < self.toks.len() {
            return Err(TypeError::ParseError(format!(
                "unexpected token at end: {:?}",
                self.toks[self.pos]
            )));
        }
        Ok(())
    }

    fn check_depth(depth: usize) -> Result<(), TypeError> {
        if depth > MAX_DEPTH {
            return Err(TypeError::ParseError(format!(
                "query nesting too deep (max {MAX_DEPTH})"
            )));
        }
        Ok(())
    }

    fn parse_or(
        &mut self,
        top: &mut TraceFilters,
        depth: usize,
    ) -> Result<Option<FilterClause>, TypeError> {
        Self::check_depth(depth)?;
        let mut nodes = Vec::new();
        if let Some(node) = self.parse_and(top, depth)? {
            nodes.push(node);
        }
        while matches!(self.peek(), Some(Tok::Or)) {
            self.bump();
            let node = self
                .parse_and(top, depth)?
                .ok_or_else(|| TypeError::ParseError("expected term after OR".into()))?;
            nodes.push(node);
        }
        Ok(match nodes.len() {
            0 => None,
            1 => nodes.pop(),
            _ => Some(FilterClause::Or(nodes)),
        })
    }

    fn parse_and(
        &mut self,
        top: &mut TraceFilters,
        depth: usize,
    ) -> Result<Option<FilterClause>, TypeError> {
        let mut nodes = Vec::new();
        if let Some(node) = self.parse_unary(top, depth)? {
            nodes.push(node);
        }
        loop {
            match self.peek() {
                Some(Tok::And) => {
                    self.bump();
                    let node = self
                        .parse_unary(top, depth)?
                        .ok_or_else(|| TypeError::ParseError("expected term after AND".into()))?;
                    nodes.push(node);
                }
                Some(Tok::Or) | Some(Tok::RParen) | None => break,
                _ => match self.parse_unary(top, depth)? {
                    Some(node) => nodes.push(node),
                    None => {
                        if matches!(self.peek(), Some(Tok::Or) | Some(Tok::RParen) | None) {
                            break;
                        }
                    }
                },
            }
        }
        Ok(match nodes.len() {
            0 => None,
            1 => nodes.pop(),
            _ => Some(FilterClause::And(nodes)),
        })
    }

    fn parse_unary(
        &mut self,
        top: &mut TraceFilters,
        depth: usize,
    ) -> Result<Option<FilterClause>, TypeError> {
        if matches!(self.peek(), Some(Tok::Not)) {
            self.bump();
            let inner = self
                .parse_unary(top, depth)?
                .ok_or_else(|| TypeError::ParseError("expected term after NOT".into()))?;
            return Ok(Some(FilterClause::Not(Box::new(inner))));
        }
        self.parse_atom(top, depth)
    }

    fn parse_atom(
        &mut self,
        top: &mut TraceFilters,
        depth: usize,
    ) -> Result<Option<FilterClause>, TypeError> {
        match self.peek().cloned() {
            Some(Tok::LParen) => {
                self.bump();
                let inner = self.parse_or(top, depth + 1)?;
                match self.bump() {
                    Some(Tok::RParen) => Ok(inner),
                    other => Err(TypeError::ParseError(format!(
                        "expected ')', got {other:?}"
                    ))),
                }
            }
            Some(Tok::Quoted(s)) => {
                self.bump();
                if let Some(value) = self.parse_quoted_key_value()? {
                    return Ok(Some(FilterClause::Attr { key: s, value }));
                }
                Ok(Some(FilterClause::Phrase(s)))
            }
            Some(Tok::Word(w)) => {
                self.bump();
                classify_word(&w, top, depth)
            }
            _ => Ok(None),
        }
    }

    fn parse_quoted_key_value(&mut self) -> Result<Option<String>, TypeError> {
        match self.peek().cloned() {
            Some(Tok::Word(separator)) if separator == ":" => {
                self.bump();
                match self.bump() {
                    Some(Tok::Quoted(value)) | Some(Tok::Word(value)) => Ok(Some(value)),
                    other => Err(TypeError::ParseError(format!(
                        "expected value after quoted field separator, got {other:?}"
                    ))),
                }
            }
            Some(Tok::Word(value)) if value.starts_with(':') && value.len() > 1 => {
                self.bump();
                Ok(Some(value[1..].to_string()))
            }
            _ => Ok(None),
        }
    }
}

fn classify_word(
    word: &str,
    top: &mut TraceFilters,
    depth: usize,
) -> Result<Option<FilterClause>, TypeError> {
    if let Some(rest) = word.strip_prefix("duration") {
        if let Some((op, value)) = peel_op(rest) {
            let ms = parse_duration_ms(value)?;
            return Ok(Some(match op {
                ">" | ">=" => FilterClause::DurationMinMs(ms),
                "<" | "<=" => FilterClause::DurationMaxMs(ms),
                _ => unreachable!(),
            }));
        }
        if rest.starts_with(':') {
            return Err(TypeError::ParseError(
                "duration must use a comparison operator (>, <, >=, <=)".into(),
            ));
        }
    }

    if let Some((key, value)) = split_first_colon(word) {
        return apply_kv(key, value, top, depth);
    }

    Ok(Some(FilterClause::Phrase(word.to_string())))
}

fn peel_op(s: &str) -> Option<(&'static str, &str)> {
    if let Some(rest) = s.strip_prefix(">=") {
        return Some((">=", rest));
    }
    if let Some(rest) = s.strip_prefix("<=") {
        return Some(("<=", rest));
    }
    if let Some(rest) = s.strip_prefix('>') {
        return Some((">", rest));
    }
    if let Some(rest) = s.strip_prefix('<') {
        return Some(("<", rest));
    }
    None
}

fn split_first_colon(word: &str) -> Option<(&str, &str)> {
    let pos = word.find(':')?;
    if pos == 0 {
        return None;
    }
    Some((&word[..pos], &word[pos + 1..]))
}

fn apply_kv(
    key: &str,
    value: &str,
    top: &mut TraceFilters,
    depth: usize,
) -> Result<Option<FilterClause>, TypeError> {
    let top_only = |key: &str| {
        TypeError::ParseError(format!(
            "{key} may only appear at top level, not inside ( ... )"
        ))
    };

    match key {
        "start_time" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.start_time = Some(parse_ts(value)?);
            Ok(None)
        }
        "end_time" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.end_time = Some(parse_ts(value)?);
            Ok(None)
        }
        "cursor_start_time" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.cursor_start_time = Some(parse_ts(value)?);
            Ok(None)
        }
        "cursor_trace_id" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.cursor_trace_id = Some(value.to_string());
            Ok(None)
        }
        "direction" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.direction = Some(value.to_string());
            Ok(None)
        }
        "limit" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.limit = Some(parse_i32(value)?);
            Ok(None)
        }
        "entity_uid" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.entity_uid = Some(value.to_string());
            Ok(None)
        }
        "queue_uid" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.queue_uid = Some(value.to_string());
            Ok(None)
        }
        "trace_id" => {
            if depth > 0 {
                return Err(top_only(key));
            }
            top.trace_ids
                .get_or_insert_with(Vec::new)
                .push(value.to_string());
            Ok(None)
        }
        // `service:` is a shorthand for the canonical summary-column filter.
        // Quoted `"service":"foo"` still falls through as an attribute search.
        "service" => Ok(Some(FilterClause::Service(value.to_string()))),
        "service_name" => Ok(Some(FilterClause::Service(value.to_string()))),
        "service_namespace" => Ok(Some(FilterClause::ServiceNamespace(value.to_string()))),
        "service_version" => Ok(Some(FilterClause::ServiceVersion(value.to_string()))),
        "service_instance_id" => Ok(Some(FilterClause::ServiceInstanceId(value.to_string()))),
        "has_errors" => Ok(Some(FilterClause::HasErrors(parse_bool(value)?))),
        "status_code" => Ok(Some(FilterClause::StatusCode(parse_i32(value)?))),
        "duration_min_ms" => Ok(Some(FilterClause::DurationMinMs(parse_i64(value)?))),
        "duration_max_ms" => Ok(Some(FilterClause::DurationMaxMs(parse_i64(value)?))),
        "status" => match value {
            "error" | "err" => Ok(Some(FilterClause::HasErrors(true))),
            "ok" => Ok(Some(FilterClause::HasErrors(false))),
            other => other
                .parse::<i32>()
                .map(FilterClause::StatusCode)
                .map(Some)
                .map_err(|_| TypeError::ParseError(format!("invalid status: {value}"))),
        },
        _ => Ok(Some(FilterClause::Attr {
            key: key.to_string(),
            value: value.to_string(),
        })),
    }
}

fn parse_duration_ms(value: &str) -> Result<i64, TypeError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(TypeError::ParseError("duration value missing".into()));
    }
    let (num, multiplier): (&str, i64) = if let Some(s) = strip_suffix_ci(value, "ms") {
        (s, 1)
    } else if let Some(s) = strip_suffix_ci(value, "s") {
        (s, 1_000)
    } else if let Some(s) = strip_suffix_ci(value, "m") {
        (s, 60_000)
    } else if let Some(s) = strip_suffix_ci(value, "h") {
        (s, 3_600_000)
    } else if value.chars().all(|c| c.is_ascii_digit()) {
        (value, 1)
    } else {
        return Err(TypeError::ParseError(format!(
            "invalid duration unit in: {value}"
        )));
    };

    let n = num
        .parse::<i64>()
        .map_err(|_| TypeError::ParseError(format!("invalid duration number: {num}")))?;
    if n < 0 {
        return Err(TypeError::ParseError("duration must be >= 0".into()));
    }
    n.checked_mul(multiplier)
        .ok_or_else(|| TypeError::ParseError("duration value overflows i64".into()))
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    if s.len() < suffix.len() {
        return None;
    }
    let (head, tail) = s.split_at(s.len() - suffix.len());
    tail.eq_ignore_ascii_case(suffix).then_some(head)
}

fn parse_bool(s: &str) -> Result<bool, TypeError> {
    match s.to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(TypeError::ParseError(format!("invalid bool: {s}"))),
    }
}

fn parse_i32(s: &str) -> Result<i32, TypeError> {
    s.parse()
        .map_err(|_| TypeError::ParseError(format!("invalid i32: {s}")))
}

fn parse_i64(s: &str) -> Result<i64, TypeError> {
    s.parse()
        .map_err(|_| TypeError::ParseError(format!("invalid i64: {s}")))
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, TypeError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| TypeError::ParseError(format!("invalid RFC3339 timestamp '{s}': {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(q: &str) -> TraceFilters {
        parse_search_query(q).expect("parse")
    }

    #[test]
    fn empty_input() {
        assert!(parse("").clause.is_none());
        assert!(parse("   \t\n").clause.is_none());
    }

    #[test]
    fn bare_word_phrase() {
        assert!(matches!(parse("kafka").clause, Some(FilterClause::Phrase(s)) if s == "kafka"));
    }

    #[test]
    fn implicit_and_two_words() {
        assert!(
            matches!(parse("kafka error").clause, Some(FilterClause::And(parts)) if parts.len() == 2)
        );
    }

    #[test]
    fn explicit_boolean_precedence() {
        let f = parse("a OR b AND c");
        assert!(
            matches!(f.clause, Some(FilterClause::Or(parts)) if matches!(&parts[1], FilterClause::And(_)))
        );
    }

    #[test]
    fn grouping_overrides_precedence() {
        let f = parse("(a OR b) AND c");
        assert!(
            matches!(f.clause, Some(FilterClause::And(parts)) if matches!(&parts[0], FilterClause::Or(_)))
        );
    }

    #[test]
    fn aliases_and_not() {
        assert!(matches!(parse("a & b").clause, Some(FilterClause::And(_))));
        assert!(matches!(parse("a | b").clause, Some(FilterClause::Or(_))));
        assert!(
            matches!(parse("a -b").clause, Some(FilterClause::And(parts)) if matches!(&parts[1], FilterClause::Not(_)))
        );
    }

    #[test]
    fn quoted_and_lowercase_keywords_are_literals() {
        assert!(matches!(parse("\"AND\"").clause, Some(FilterClause::Phrase(s)) if s == "AND"));
        assert!(
            matches!(parse("foo and bar").clause, Some(FilterClause::And(parts)) if parts.len() == 3)
        );
    }

    #[test]
    fn structured_fields() {
        assert!(
            matches!(parse("service:foo").clause, Some(FilterClause::Service(s)) if s == "foo")
        );
        assert!(
            matches!(parse("service_name:foo").clause, Some(FilterClause::Service(s)) if s == "foo")
        );
        assert!(
            matches!(parse("\"service\":\"foo\"").clause, Some(FilterClause::Attr { key, value }) if key == "service" && value == "foo")
        );
        assert!(
            matches!(parse("component:kafka").clause, Some(FilterClause::Attr { key, value }) if key == "component" && value == "kafka")
        );
        assert!(
            matches!(parse("http.url:http://x.com/path").clause, Some(FilterClause::Attr { key, value }) if key == "http.url" && value == "http://x.com/path")
        );
    }

    #[test]
    fn durations_and_status() {
        assert!(matches!(
            parse("duration>500ms").clause,
            Some(FilterClause::DurationMinMs(500))
        ));
        assert!(matches!(
            parse("duration<=2s").clause,
            Some(FilterClause::DurationMaxMs(2000))
        ));
        assert!(matches!(
            parse("duration>=1m").clause,
            Some(FilterClause::DurationMinMs(60000))
        ));
        assert!(parse_search_query("duration:500ms").is_err());
        assert!(matches!(
            parse("status:error").clause,
            Some(FilterClause::HasErrors(true))
        ));
        assert!(matches!(
            parse("status:ok").clause,
            Some(FilterClause::HasErrors(false))
        ));
        assert!(matches!(
            parse("status:2").clause,
            Some(FilterClause::StatusCode(2))
        ));
    }

    #[test]
    fn top_level_fields_routed_and_rejected_in_groups() {
        let f = parse("start_time:2026-05-05T00:00:00Z entity_uid:abc component:kafka");
        assert!(f.start_time.is_some());
        assert_eq!(f.entity_uid.as_deref(), Some("abc"));
        assert!(f.clause.is_some());
        assert!(parse_search_query("(start_time:2026-05-05T00:00:00Z foo)").is_err());
    }

    #[test]
    fn trace_id_appends_top_level() {
        let f = parse("trace_id:abc trace_id:def");
        assert_eq!(
            f.trace_ids.as_deref().unwrap(),
            &["abc".to_string(), "def".to_string()]
        );
        assert!(f.clause.is_none());
    }

    #[test]
    fn malformed_queries_error() {
        assert!(parse_search_query("\"unterminated").is_err());
        assert!(parse_search_query("(a OR b").is_err());
        assert!(parse_search_query("a OR b)").is_err());
        let q = "(".repeat(40) + "foo" + &")".repeat(40);
        assert!(parse_search_query(&q).is_err());
    }

    #[test]
    fn validates_direct_clause_complexity() {
        let mut deep = FilterClause::Phrase("leaf".into());
        for _ in 0..=MAX_DEPTH {
            deep = FilterClause::Not(Box::new(deep));
        }
        assert!(validate_filter_clause(&deep).is_err());

        let wide = FilterClause::Or(
            (0..=MAX_CLAUSE_FANOUT)
                .map(|idx| FilterClause::Phrase(format!("leaf-{idx}")))
                .collect(),
        );
        assert!(validate_filter_clause(&wide).is_err());

        let large_leaf = FilterClause::Phrase("x".repeat(MAX_CLAUSE_LEAF_STRING_LEN + 1));
        assert!(validate_filter_clause(&large_leaf).is_err());

        let too_many_nodes = FilterClause::And(
            (0..MAX_CLAUSE_FANOUT)
                .map(|branch| {
                    FilterClause::And(
                        (0..8)
                            .map(|leaf| FilterClause::Phrase(format!("leaf-{branch}-{leaf}")))
                            .collect(),
                    )
                })
                .collect(),
        );
        assert!(validate_filter_clause(&too_many_nodes).is_err());
    }

    #[test]
    fn validates_direct_clause_duration_range() {
        let clause = FilterClause::And(vec![
            FilterClause::DurationMinMs(500),
            FilterClause::DurationMaxMs(100),
        ]);

        assert!(validate_filter_clause(&clause).is_err());
    }

    #[test]
    fn validates_direct_clause_nested_conjunctive_duration_range() {
        let clause = FilterClause::And(vec![
            FilterClause::DurationMinMs(500),
            FilterClause::And(vec![FilterClause::DurationMaxMs(100)]),
        ]);

        assert!(validate_filter_clause(&clause).is_err());
    }

    #[test]
    fn validates_direct_clause_duration_non_negative() {
        assert!(validate_filter_clause(&FilterClause::DurationMinMs(-1)).is_err());
        assert!(validate_filter_clause(&FilterClause::DurationMaxMs(-1)).is_err());
    }

    #[test]
    fn accepts_direct_clause_disjunctive_duration_alternatives() {
        let clause = FilterClause::Or(vec![
            FilterClause::DurationMinMs(500),
            FilterClause::DurationMaxMs(100),
        ]);

        assert!(validate_filter_clause(&clause).is_ok());
    }
}
