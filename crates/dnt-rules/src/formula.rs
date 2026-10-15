//! The formula language: a small, pure expression language used by
//! automation overlays and stat definitions.
//!
//! ```text
//! expr    := or
//! or      := and ("or" and)*
//! and     := not ("and" not)*
//! not     := "not" not | cmp
//! cmp     := sum (("==" | "!=" | "<" | "<=" | ">" | ">=") sum)?
//! sum     := product (("+" | "-") product)*
//! product := unary (("*" | "/") unary)*
//! unary   := "-" unary | primary
//! primary := number | dice | string | "true" | "false"
//!          | path | path "(" args ")" | "(" expr ")" | "{" key ":" expr, ... "}"
//! ```
//!
//! Paths are dotted identifiers (`class.monk.martial_arts`). Dice literals
//! are `NdF` or `dF`. Division is exact; stats round down at the end.

use std::fmt;
use std::ops::Range;

use crate::value::{Dice, Num, Value};

pub type Span = Range<usize>;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub message: String,
    /// Byte range in the formula source.
    pub span: Span,
}

impl Error {
    fn new(message: impl Into<String>, span: Span) -> Error {
        Error { message: message.into(), span }
    }

    /// Render the error with a caret line under the offending source.
    pub fn display(&self, source: &str) -> String {
        let start = source[..self.span.start.min(source.len())].chars().count();
        let len = source.get(self.span.clone()).map_or(1, |s| s.chars().count().max(1));
        format!("{}\n{source}\n{}{}", self.message, " ".repeat(start), "^".repeat(len))
    }
}

// ---- lexer ----

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Dice(i64, u32),
    Str(String),
    Path(String),
    True,
    False,
    And,
    Or,
    Not,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

fn lex(src: &str) -> Result<Vec<(Tok, Span)>, Error> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    while i < bytes.len() {
        let c = bytes[i];
        let start = i;
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let tok = if c.is_ascii_digit() {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let n: i64 = src[start..i].parse().map_err(|_| Error::new("number too large", start..i))?;
            // `2d6`
            if i < bytes.len() && bytes[i] == b'd' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                i += 1;
                let fs = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let faces = src[fs..i].parse().map_err(|_| Error::new("die too large", start..i))?;
                if i < bytes.len() && is_ident(bytes[i]) {
                    return Err(Error::new("malformed dice", start..i + 1));
                }
                Tok::Dice(n, faces)
            } else if i < bytes.len() && is_ident(bytes[i]) {
                return Err(Error::new("identifiers cannot start with a digit", start..i + 1));
            } else {
                Tok::Num(n)
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            // A dotted path; segments may start with digits after a dot.
            while i < bytes.len() && (is_ident(bytes[i]) || (bytes[i] == b'.' && i + 1 < bytes.len() && is_ident(bytes[i + 1]))) {
                i += 1;
            }
            let word = &src[start..i];
            match word {
                "true" => Tok::True,
                "false" => Tok::False,
                "and" => Tok::And,
                "or" => Tok::Or,
                "not" => Tok::Not,
                w if w.len() > 1 && w.starts_with('d') && w[1..].bytes().all(|b| b.is_ascii_digit()) => {
                    Tok::Dice(1, w[1..].parse().map_err(|_| Error::new("die too large", start..i))?)
                }
                w => Tok::Path(w.to_string()),
            }
        } else if c == b'\'' || c == b'"' {
            i += 1;
            let s = i;
            while i < bytes.len() && bytes[i] != c {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(Error::new("unterminated string", start..bytes.len()));
            }
            i += 1;
            Tok::Str(src[s..i - 1].to_string())
        } else {
            let two = src.get(i..i + 2).unwrap_or("");
            let (tok, len) = match (c, two) {
                (_, "==") => (Tok::Eq, 2),
                (_, "!=") => (Tok::Ne, 2),
                (_, "<=") => (Tok::Le, 2),
                (_, ">=") => (Tok::Ge, 2),
                (_, "&&") => (Tok::And, 2),
                (_, "||") => (Tok::Or, 2),
                (b'<', _) => (Tok::Lt, 1),
                (b'>', _) => (Tok::Gt, 1),
                (b'!', _) => (Tok::Not, 1),
                (b'(', _) => (Tok::LParen, 1),
                (b')', _) => (Tok::RParen, 1),
                (b'{', _) => (Tok::LBrace, 1),
                (b'}', _) => (Tok::RBrace, 1),
                (b',', _) => (Tok::Comma, 1),
                (b':', _) => (Tok::Colon, 1),
                (b'+', _) => (Tok::Plus, 1),
                (b'-', _) => (Tok::Minus, 1),
                (b'*', _) => (Tok::Star, 1),
                (b'/', _) => (Tok::Slash, 1),
                _ => {
                    let ch_len = src[i..].chars().next().map_or(1, char::len_utf8);
                    return Err(Error::new("unexpected character", i..i + ch_len));
                }
            };
            i += len;
            tok
        };
        out.push((tok, start..i));
    }
    Ok(out)
}

// ---- AST ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Lit(Value),
    Var(String),
    Call(String, Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

// ---- parser ----

struct Parser<'a> {
    toks: &'a [(Tok, Span)],
    pos: usize,
    len: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(t, _)| t)
    }

    fn span(&self) -> Span {
        self.toks.get(self.pos).map_or(self.len..self.len, |(_, s)| s.clone())
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: &Tok, what: &str) -> Result<Span, Error> {
        let span = self.span();
        if self.eat(tok) {
            Ok(span)
        } else {
            Err(Error::new(format!("expected {what}"), span))
        }
    }

    fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
        let span = l.span.start..r.span.end;
        Expr { kind: ExprKind::Bin(op, Box::new(l), Box::new(r)), span }
    }

    fn or(&mut self) -> Result<Expr, Error> {
        let mut l = self.and()?;
        while self.eat(&Tok::Or) {
            let r = self.and()?;
            l = Self::bin(BinOp::Or, l, r);
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Expr, Error> {
        let mut l = self.not()?;
        while self.eat(&Tok::And) {
            let r = self.not()?;
            l = Self::bin(BinOp::And, l, r);
        }
        Ok(l)
    }

    fn not(&mut self) -> Result<Expr, Error> {
        let start = self.span().start;
        if self.eat(&Tok::Not) {
            let e = self.not()?;
            let span = start..e.span.end;
            return Ok(Expr { kind: ExprKind::Not(Box::new(e)), span });
        }
        self.cmp()
    }

    fn cmp(&mut self) -> Result<Expr, Error> {
        let l = self.sum()?;
        let op = match self.peek() {
            Some(Tok::Eq) => BinOp::Eq,
            Some(Tok::Ne) => BinOp::Ne,
            Some(Tok::Lt) => BinOp::Lt,
            Some(Tok::Le) => BinOp::Le,
            Some(Tok::Gt) => BinOp::Gt,
            Some(Tok::Ge) => BinOp::Ge,
            _ => return Ok(l),
        };
        self.pos += 1;
        let r = self.sum()?;
        Ok(Self::bin(op, l, r))
    }

    fn sum(&mut self) -> Result<Expr, Error> {
        let mut l = self.product()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Ok(l),
            };
            self.pos += 1;
            let r = self.product()?;
            l = Self::bin(op, l, r);
        }
    }

    fn product(&mut self) -> Result<Expr, Error> {
        let mut l = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => return Ok(l),
            };
            self.pos += 1;
            let r = self.unary()?;
            l = Self::bin(op, l, r);
        }
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        let start = self.span().start;
        if self.eat(&Tok::Minus) {
            let e = self.unary()?;
            let span = start..e.span.end;
            return Ok(Expr { kind: ExprKind::Neg(Box::new(e)), span });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let span = self.span();
        let Some(tok) = self.peek().cloned() else {
            return Err(Error::new("unexpected end of formula", span));
        };
        self.pos += 1;
        let lit = |v: Value| Ok(Expr { kind: ExprKind::Lit(v), span: span.clone() });
        match tok {
            Tok::Num(n) => lit(Value::int(n)),
            Tok::Dice(n, f) => lit(Value::Dice(Dice::new(n, f))),
            Tok::Str(s) => lit(Value::Str(s)),
            Tok::True => lit(Value::Bool(true)),
            Tok::False => lit(Value::Bool(false)),
            Tok::Path(name) => {
                if self.eat(&Tok::LParen) {
                    let mut args = Vec::new();
                    if !self.eat(&Tok::RParen) {
                        loop {
                            args.push(self.or()?);
                            if self.eat(&Tok::RParen) {
                                break;
                            }
                            self.expect(&Tok::Comma, "`,` or `)`")?;
                        }
                    }
                    let end = self.toks[self.pos - 1].1.end;
                    Ok(Expr { kind: ExprKind::Call(name, args), span: span.start..end })
                } else {
                    Ok(Expr { kind: ExprKind::Var(name), span })
                }
            }
            Tok::LParen => {
                let e = self.or()?;
                self.expect(&Tok::RParen, "`)`")?;
                Ok(e)
            }
            Tok::LBrace => {
                let mut entries = Vec::new();
                if !self.eat(&Tok::RBrace) {
                    loop {
                        let k = self.or()?;
                        self.expect(&Tok::Colon, "`:`")?;
                        let v = self.or()?;
                        entries.push((k, v));
                        if self.eat(&Tok::RBrace) {
                            break;
                        }
                        self.expect(&Tok::Comma, "`,` or `}`")?;
                    }
                }
                let end = self.toks[self.pos - 1].1.end;
                Ok(Expr { kind: ExprKind::Map(entries), span: span.start..end })
            }
            _ => Err(Error::new("expected a value", span)),
        }
    }
}

/// A parsed formula together with its source text.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub source: String,
    pub expr: Expr,
}

impl fmt::Display for Formula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

impl Formula {
    pub fn parse(source: &str) -> Result<Formula, Error> {
        let toks = lex(source)?;
        let mut p = Parser { toks: &toks, pos: 0, len: source.len() };
        let expr = p.or()?;
        if p.pos < toks.len() {
            return Err(Error::new("unexpected input after the formula", p.span()));
        }
        Ok(Formula { source: source.to_string(), expr })
    }

    /// A formula that is just a constant.
    pub fn constant(v: Value) -> Formula {
        Formula { source: v.to_string(), expr: Expr { kind: ExprKind::Lit(v), span: 0..0 } }
    }

    /// Every variable this formula reads, including the implicit ones read
    /// by `toggle('x')` (as `toggle.x`).
    pub fn vars(&self) -> Vec<String> {
        fn walk(e: &Expr, out: &mut Vec<String>) {
            match &e.kind {
                ExprKind::Var(v) => out.push(v.clone()),
                ExprKind::Call(name, args) => {
                    if name == "toggle" {
                        if let Some(ExprKind::Lit(Value::Str(k))) = args.first().map(|a| &a.kind) {
                            out.push(format!("toggle.{k}"));
                        }
                    }
                    args.iter().for_each(|a| walk(a, out));
                }
                ExprKind::Map(entries) => entries.iter().for_each(|(k, v)| {
                    walk(k, out);
                    walk(v, out);
                }),
                ExprKind::Neg(e) | ExprKind::Not(e) => walk(e, out),
                ExprKind::Bin(_, l, r) => {
                    walk(l, out);
                    walk(r, out);
                }
                ExprKind::Lit(_) => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.expr, &mut out);
        out.sort();
        out.dedup();
        out
    }

    pub fn eval(&self, env: &dyn Env) -> Result<Value, Error> {
        eval(&self.expr, env)
    }
}

/// Supplies variables and host functions to formulas.
pub trait Env {
    fn var(&self, name: &str) -> Option<Value>;

    /// Host functions (e.g. `level('Wizard')`). `None` means "no such function".
    fn call(&self, _name: &str, _args: &[Value]) -> Option<Result<Value, String>> {
        None
    }
}

/// An environment backed by a map; handy for tests and previews.
impl Env for std::collections::HashMap<String, Value> {
    fn var(&self, name: &str) -> Option<Value> {
        self.get(name).cloned()
    }
}

// ---- evaluation ----

fn num(v: &Value, span: &Span) -> Result<Num, Error> {
    v.as_num().ok_or_else(|| Error::new(format!("expected a number, found {}", v.type_name()), span.clone()))
}

fn int(v: &Value, span: &Span) -> Result<i64, Error> {
    let n = num(v, span)?;
    if n.is_int() {
        Ok(n.floor())
    } else {
        Err(Error::new(format!("expected a whole number, found {n}"), span.clone()))
    }
}

fn boolean(v: &Value, span: &Span) -> Result<bool, Error> {
    v.as_bool().ok_or_else(|| Error::new(format!("expected true/false, found {}", v.type_name()), span.clone()))
}

fn eval(e: &Expr, env: &dyn Env) -> Result<Value, Error> {
    match &e.kind {
        ExprKind::Lit(v) => Ok(v.clone()),
        ExprKind::Var(name) => env.var(name).ok_or_else(|| Error::new(format!("unknown variable `{name}`"), e.span.clone())),
        ExprKind::Map(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                out.push((eval(k, env)?, eval(v, env)?));
            }
            Ok(Value::Map(out))
        }
        ExprKind::Neg(inner) => match eval(inner, env)? {
            Value::Num(n) => Ok(Value::Num(-n)),
            Value::Dice(d) => Ok(Value::Dice(d.times(-1))),
            v => Err(Error::new(format!("cannot negate a {}", v.type_name()), inner.span.clone())),
        },
        ExprKind::Not(inner) => Ok(Value::Bool(!boolean(&eval(inner, env)?, &inner.span)?)),
        ExprKind::Bin(op, l, r) => {
            // Short-circuit logic.
            if matches!(op, BinOp::And | BinOp::Or) {
                let lv = boolean(&eval(l, env)?, &l.span)?;
                if (*op == BinOp::And && !lv) || (*op == BinOp::Or && lv) {
                    return Ok(Value::Bool(lv));
                }
                return Ok(Value::Bool(boolean(&eval(r, env)?, &r.span)?));
            }
            let lv = eval(l, env)?;
            let rv = eval(r, env)?;
            binary(*op, lv, rv, l, r, &e.span)
        }
        ExprKind::Call(name, args) => call(name, args, env, &e.span),
    }
}

fn binary(op: BinOp, lv: Value, rv: Value, l: &Expr, r: &Expr, span: &Span) -> Result<Value, Error> {
    use Value::{Dice as D, Num as N};
    let mismatch = || Error::new(format!("cannot apply `{}` to {} and {}", op_symbol(op), lv.type_name(), rv.type_name()), span.clone());
    Ok(match (op, &lv, &rv) {
        (BinOp::Add, N(a), N(b)) => N(*a + *b),
        (BinOp::Sub, N(a), N(b)) => N(*a - *b),
        (BinOp::Mul, N(a), N(b)) => N(*a * *b),
        (BinOp::Div, N(a), N(b)) => N(a.checked_div(*b).ok_or_else(|| Error::new("division by zero", r.span.clone()))?),
        (BinOp::Add, D(a), D(b)) => D(a.plus(b)),
        (BinOp::Sub, D(a), D(b)) => D(a.plus(&b.times(-1))),
        (BinOp::Add, D(d), N(_)) => D(d.plus_flat(int(&rv, &r.span)?)),
        (BinOp::Add, N(_), D(d)) => D(d.plus_flat(int(&lv, &l.span)?)),
        (BinOp::Sub, D(d), N(_)) => D(d.plus_flat(-int(&rv, &r.span)?)),
        (BinOp::Sub, N(_), D(d)) => D(d.times(-1).plus_flat(int(&lv, &l.span)?)),
        (BinOp::Mul, D(d), N(_)) => D(d.times(int(&rv, &r.span)?)),
        (BinOp::Mul, N(_), D(d)) => D(d.times(int(&lv, &l.span)?)),
        (BinOp::Lt, N(a), N(b)) => Value::Bool(a < b),
        (BinOp::Le, N(a), N(b)) => Value::Bool(a <= b),
        (BinOp::Gt, N(a), N(b)) => Value::Bool(a > b),
        (BinOp::Ge, N(a), N(b)) => Value::Bool(a >= b),
        (BinOp::Eq, a, b) if a.type_name() == b.type_name() => Value::Bool(eq_ci(a, b)),
        (BinOp::Ne, a, b) if a.type_name() == b.type_name() => Value::Bool(!eq_ci(a, b)),
        _ => return Err(mismatch()),
    })
}

/// Equality, ignoring case for strings (class names, ability names, …).
fn eq_ci(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(a), Value::Str(b)) => a.eq_ignore_ascii_case(b),
        _ => a == b,
    }
}

fn op_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

/// Built-in function names, for documentation and completion.
pub const BUILTINS: &[(&str, &str)] = &[
    ("mod(score)", "ability modifier: floor((score - 10) / 2)"),
    ("min(a, b, …)", "smallest number"),
    ("max(a, b, …)", "largest number"),
    ("floor(x)", "round down"),
    ("ceil(x)", "round up"),
    ("abs(x)", "absolute value"),
    ("if(cond, then, else)", "choose a value; only the chosen branch is evaluated"),
    ("scale(n, {lvl: value, …})", "value for the highest key ≤ n (0 if none)"),
    ("dice(count, faces)", "build dice, e.g. dice(level / 2, 6)"),
    ("average(dice)", "average roll, rounded down"),
    ("toggle('key')", "whether a toggle is on"),
];

fn call(name: &str, args: &[Expr], env: &dyn Env, span: &Span) -> Result<Value, Error> {
    let arity = |n: usize| {
        if args.len() == n {
            Ok(())
        } else {
            Err(Error::new(format!("`{name}` takes {n} argument(s), got {}", args.len()), span.clone()))
        }
    };
    match name {
        // Lazy: only the taken branch is evaluated.
        "if" => {
            arity(3)?;
            let c = boolean(&eval(&args[0], env)?, &args[0].span)?;
            eval(&args[if c { 1 } else { 2 }], env)
        }
        "toggle" => {
            arity(1)?;
            let key = match eval(&args[0], env)? {
                Value::Str(s) => s,
                v => return Err(Error::new(format!("expected a toggle name, found {}", v.type_name()), args[0].span.clone())),
            };
            Ok(env.var(&format!("toggle.{key}")).unwrap_or(Value::Bool(false)))
        }
        _ => {
            let vals: Vec<Value> = args.iter().map(|a| eval(a, env)).collect::<Result<_, _>>()?;
            builtin(name, &vals, args, span)
                .or_else(|| env.call(name, &vals).map(|r| r.map_err(|m| Error::new(m, span.clone()))))
                .unwrap_or_else(|| Err(Error::new(format!("unknown function `{name}`"), span.clone())))
        }
    }
}

fn builtin(name: &str, vals: &[Value], args: &[Expr], span: &Span) -> Option<Result<Value, Error>> {
    if !matches!(name, "mod" | "min" | "max" | "floor" | "ceil" | "abs" | "scale" | "dice" | "average") {
        return None;
    }
    let arity = |n: usize| {
        if vals.len() == n {
            Ok(())
        } else {
            Err(Error::new(format!("`{name}` takes {n} argument(s), got {}", vals.len()), span.clone()))
        }
    };
    let n_at = |i: usize| num(&vals[i], &args[i].span);
    Some((|| -> Result<Value, Error> {
        match name {
            "mod" => {
                arity(1)?;
                Ok(Value::int((n_at(0)? - Num::int(10)).checked_div(Num::int(2)).unwrap().floor()))
            }
            "min" | "max" => {
                if vals.is_empty() {
                    return Err(Error::new(format!("`{name}` needs at least one argument"), span.clone()));
                }
                let mut best = n_at(0)?;
                for i in 1..vals.len() {
                    let v = n_at(i)?;
                    if (name == "min" && v < best) || (name == "max" && v > best) {
                        best = v;
                    }
                }
                Ok(Value::Num(best))
            }
            "floor" => {
                arity(1)?;
                Ok(Value::int(n_at(0)?.floor()))
            }
            "ceil" => {
                arity(1)?;
                Ok(Value::int(n_at(0)?.ceil()))
            }
            "abs" => {
                arity(1)?;
                let n = n_at(0)?;
                Ok(Value::Num(if n < Num::ZERO { -n } else { n }))
            }
            "scale" => {
                arity(2)?;
                let level = n_at(0)?;
                let Value::Map(entries) = &vals[1] else {
                    return Err(Error::new("expected a map like {1: 2, 5: 3}", args[1].span.clone()));
                };
                let mut best: Option<(Num, &Value)> = None;
                for (k, v) in entries {
                    let k = num(k, &args[1].span)?;
                    if k <= level && best.is_none_or(|(b, _)| k > b) {
                        best = Some((k, v));
                    }
                }
                Ok(best.map_or(Value::int(0), |(_, v)| v.clone()))
            }
            "dice" => {
                arity(2)?;
                let count = n_at(0)?.floor();
                let faces = int(&vals[1], &args[1].span)?;
                if !(1..=1000).contains(&faces) {
                    return Err(Error::new("die faces must be between 1 and 1000", args[1].span.clone()));
                }
                Ok(Value::Dice(Dice::new(count, faces as u32)))
            }
            "average" => {
                arity(1)?;
                match &vals[0] {
                    Value::Dice(d) => Ok(Value::int(d.average())),
                    Value::Num(n) => Ok(Value::Num(*n)),
                    v => Err(Error::new(format!("expected dice, found {}", v.type_name()), args[0].span.clone())),
                }
            }
            _ => unreachable!("checked above"),
        }
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env() -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert("level".into(), Value::int(5));
        m.insert("dex".into(), Value::int(16));
        m.insert("prof".into(), Value::int(3));
        m.insert("armor.heavy".into(), Value::Bool(false));
        m.insert("class.monk.martial_arts".into(), Value::Dice(Dice::new(1, 8)));
        m.insert("toggle.rage".into(), Value::Bool(true));
        m
    }

    fn ev(src: &str) -> Value {
        Formula::parse(src).unwrap_or_else(|e| panic!("{}", e.display(src))).eval(&env()).unwrap_or_else(|e| panic!("{}", e.display(src)))
    }

    fn err(src: &str) -> Error {
        match Formula::parse(src) {
            Err(e) => e,
            Ok(f) => f.eval(&env()).unwrap_err(),
        }
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(ev("1 + 2 * 3"), Value::int(7));
        assert_eq!(ev("(1 + 2) * 3"), Value::int(9));
        assert_eq!(ev("-2 + 5"), Value::int(3));
        assert_eq!(ev("10 - 4 - 3"), Value::int(3));
        assert_eq!(ev("level / 2"), Value::Num(Num::ratio(5, 2).unwrap()));
        assert_eq!(ev("ceil(level / 2)"), Value::int(3));
        assert_eq!(ev("floor(level / 2)"), Value::int(2));
    }

    #[test]
    fn variables_and_builtins() {
        assert_eq!(ev("mod(dex)"), Value::int(3));
        assert_eq!(ev("mod(9)"), Value::int(-1));
        assert_eq!(ev("10 + mod(dex) + prof"), Value::int(16));
        assert_eq!(ev("max(1, mod(dex))"), Value::int(3));
        assert_eq!(ev("min(2, 7, -1)"), Value::int(-1));
        assert_eq!(ev("scale(level, {1: 1, 5: 2, 11: 3})"), Value::int(2));
        assert_eq!(ev("scale(level, {6: 1})"), Value::int(0));
    }

    #[test]
    fn dice() {
        assert_eq!(ev("class.monk.martial_arts + mod(dex)").to_string(), "1d8 + 3");
        assert_eq!(ev("2 * d6").to_string(), "2d6");
        assert_eq!(ev("1d6 + 2d6").to_string(), "3d6");
        assert_eq!(ev("dice(ceil(level / 2), 6)").to_string(), "3d6");
        assert_eq!(ev("average(2d6 + 1)"), Value::int(8));
        assert_eq!(ev("scale(level, {1: d6, 5: d8})").to_string(), "1d8");
    }

    #[test]
    fn logic() {
        assert_eq!(ev("not armor.heavy and level >= 5"), Value::Bool(true));
        assert_eq!(ev("toggle('rage')"), Value::Bool(true));
        assert_eq!(ev("toggle('unknown')"), Value::Bool(false));
        assert_eq!(ev("if(toggle('rage'), 2, 0)"), Value::int(2));
        // Lazy `if` and short-circuit: the bad branch is never evaluated.
        assert_eq!(ev("if(true, 1, nope)"), Value::int(1));
        assert_eq!(ev("false and nope"), Value::Bool(false));
        assert_eq!(ev("'Wizard' == 'wizard'"), Value::Bool(true));
    }

    #[test]
    fn errors_have_spans() {
        let e = err("10 + nope");
        assert_eq!(e.message, "unknown variable `nope`");
        assert_eq!(e.span, 5..9);
        assert_eq!(err("1 +").message, "unexpected end of formula");
        assert_eq!(err("mod(1, 2)").message, "`mod` takes 1 argument(s), got 2");
        assert_eq!(err("1 / 0").message, "division by zero");
        assert_eq!(err("armor.heavy + 1").message, "cannot apply `+` to boolean and number");
        assert_eq!(err("3dx").message, "identifiers cannot start with a digit");
        assert_eq!(err("1 2").message, "unexpected input after the formula");
        assert_eq!(err("frobnicate(1)").message, "unknown function `frobnicate`");
        assert!(err("10 + nope").display("10 + nope").ends_with("     ^^^^"));
    }

    #[test]
    fn collects_vars() {
        let f = Formula::parse("10 + mod(dex) + if(toggle('rage'), prof, 0) + class.monk.martial_arts").unwrap();
        assert_eq!(f.vars(), vec!["class.monk.martial_arts", "dex", "prof", "toggle.rage"]);
    }
}
