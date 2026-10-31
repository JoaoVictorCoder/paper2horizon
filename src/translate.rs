//! Method bodies, from bytecode to Rust: a listener's, a command's, a task's.
//!
//! The bytecode is run symbolically: each instruction pushes or pops typed expressions, and calls,
//! stores and returns become statements. `if`/`else` comes back from forward branches. A method
//! translates whole or not at all: a loop, a `try`, a call with no Rust form or a type that doesn't
//! fit gives up with the reason, and the method stays a `TODO` listing its calls.

use crate::classfile::{self, Body, Class, Const, Indy, MemberRef, Method};
use crate::mapping;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// What a Rust expression is, as far as the translation needs to know.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A `Player` (a value, a field, or `&Player`).
    Player,
    /// A `&CommandSender` (the console or a player).
    Sender,
    /// A `String`.
    Str,
    /// A `&str`.
    StrRef,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Char,
    Location,
    Block,
    BlockPos,
    World,
    /// `ctx.config()`.
    Config,
    /// `ctx.logger()`.
    Logger,
    /// `server()`.
    Server,
    /// `ctx.description()`.
    Description,
    Color,
    GameMode,
    /// The event a listener got, by its Horizon name.
    Event(&'static str),
    /// The plugin's struct (`plugin`).
    Plugin,
    /// A listener class other than the plugin's, whose fields are only a reference to the plugin.
    Listener,
    /// The command a command executor got (Bukkit's `Command`).
    Command,
    /// A command's arguments, `&[String]`.
    Args,
    /// A `Vec<String>`.
    StrList,
    /// A `Vec<Player>`.
    PlayerList,
    Uuid,
    /// A collection's iterator (in a for-each loop), by what it iterates and the items' kind.
    Iter(String, Box<Kind>),
    /// `iterator.hasNext()`: the loop's condition.
    HasNext(String, Box<Kind>),
    /// `iterator.next()`: the loop's variable.
    LoopItem(Box<Kind>),
    /// `text.hashCode()`, which only a string switch reads.
    Hash(Box<Expr>),
    /// Java's `null`.
    Null,
    /// A `new` whose constructor hasn't run yet.
    New(String),
    /// A `StringBuilder` and what was appended to it.
    Builder(Vec<Expr>),
    /// A `new String[n]` being filled.
    Array(Vec<Option<Expr>>),
    /// What `lcmp`, `dcmpl`... leave for the branch after them.
    Compare(Box<Expr>, Box<Expr>),
    /// What Rust gives back that Java code may ignore (a count, a "queued" flag).
    Ignored,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub code: String,
    pub kind: Kind,
    /// Safe to put a method call or an operator after without parentheses.
    pub atom: bool,
    /// A place (`e.player`, `plugin.name`): moving it out needs `.clone()`.
    pub place: bool,
    /// For literals: the value.
    pub lit: Option<Lit>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    Str(String),
    Int(i64),
    Float(f64),
}

impl Expr {
    fn new(code: impl Into<String>, kind: Kind) -> Expr {
        Expr { code: code.into(), kind, atom: true, place: false, lit: None }
    }
    fn op(code: impl Into<String>, kind: Kind) -> Expr {
        Expr { code: code.into(), kind, atom: false, place: false, lit: None }
    }
    fn place(code: impl Into<String>, kind: Kind) -> Expr {
        Expr { code: code.into(), kind, atom: true, place: true, lit: None }
    }
    fn int(v: i64, kind: Kind) -> Expr {
        let code = if v < 0 { format!("({v})") } else { v.to_string() };
        Expr { code, kind, atom: true, place: false, lit: Some(Lit::Int(v)) }
    }
    fn float(v: f64, kind: Kind) -> Expr {
        let mut code = format!("{v:?}");
        if kind == Kind::F32 {
            code.push_str("_f32");
        }
        if v < 0.0 {
            code = format!("({code})");
        }
        Expr { code, kind, atom: true, place: false, lit: Some(Lit::Float(v)) }
    }
    fn string(s: &str) -> Expr {
        Expr { code: format!("{s:?}"), kind: Kind::StrRef, atom: true, place: false, lit: Some(Lit::Str(s.to_owned())) }
    }
    /// For a method call or field after it.
    fn recv(&self) -> String {
        if self.atom { self.code.clone() } else { format!("({})", self.code) }
    }
}

/// What a parameter slot holds.
#[derive(Clone, Debug)]
pub enum Param {
    Event(&'static str),
    Sender,
    Command,
    Label,
    Args,
    /// A helper's parameter: its Rust name and kind.
    Named(String, Kind),
}

/// What the method returns, in Rust.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ret {
    Unit,
    Bool,
    StrList,
    /// A helper's value: `String`, a number.
    Value(ValueKind),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueKind {
    Str,
    I32,
    I64,
    F64,
}

/// A method of the plugin's own that was translated into a Rust function, as its callers see it.
#[derive(Clone, Debug)]
pub struct Helper {
    pub name: String,
    pub params: Vec<Kind>,
    /// What it gives back; `None` for nothing.
    pub ret: Option<Kind>,
}

/// Finds (translating it if need be) the helper for a call to one of the plugin's own methods; `Err`
/// says why there is none.
pub type Helpers<'a> = &'a dyn Fn(&MemberRef) -> Result<Helper, String>;

/// A helper's parameters and return as kinds, from its descriptor; `None` when one has no Rust form.
pub fn helper_kinds(descriptor: &str) -> Option<(Vec<Kind>, Option<Kind>)> {
    let kind = |t: &str| -> Option<Kind> {
        Some(match t {
            "I" | "S" | "B" => Kind::I32,
            "J" => Kind::I64,
            "D" => Kind::F64,
            "F" => Kind::F32,
            "Z" => Kind::Bool,
            "java/lang/String" => Kind::StrRef,
            "[java/lang/String" => Kind::Args,
            "org/bukkit/entity/Player" => Kind::Player,
            "org/bukkit/command/CommandSender" => Kind::Sender,
            "org/bukkit/Location" => Kind::Location,
            "org/bukkit/World" => Kind::World,
            "org/bukkit/block/Block" => Kind::Block,
            _ => return None,
        })
    };
    let params = classfile::parameters(descriptor).iter().map(|t| kind(t)).collect::<Option<Vec<_>>>()?;
    let ret = match ret_letter(descriptor) {
        "V" => None,
        "Z" => Some(Kind::Bool),
        "I" | "S" | "B" => Some(Kind::I32),
        "J" => Some(Kind::I64),
        "D" => Some(Kind::F64),
        "Ljava/lang/String;" => Some(Kind::Str),
        _ => return None,
    };
    Some((params, ret))
}

/// A parameter's Rust type.
pub fn param_type(k: &Kind) -> &'static str {
    match k {
        Kind::I32 => "i32",
        Kind::I64 => "i64",
        Kind::F64 => "f64",
        Kind::F32 => "f32",
        Kind::Bool => "bool",
        Kind::StrRef => "&str",
        Kind::Args => "&[String]",
        Kind::Player => "&Player",
        Kind::Sender => "&CommandSender",
        Kind::Location => "&Location",
        Kind::World => "&World",
        Kind::Block => "&Block",
        Kind::Str => "String",
        _ => "()",
    }
}

/// A method's parameters' names: the class file's, else `a1`, `a2`...
pub fn param_names(m: &Method) -> Vec<String> {
    let first = if m.is_static { 0 } else { 1 };
    let mut slot = first;
    let mut out: Vec<String> = Vec::new();
    for (i, t) in classfile::parameters(&m.descriptor).iter().enumerate() {
        let java = m
            .body
            .as_ref()
            .and_then(|b| b.locals.iter().find(|l| l.slot == slot && l.start == 0))
            .map(|l| l.name.clone());
        let mut name = java.map(|n| snake(&n)).unwrap_or_else(|| format!("a{}", i + 1));
        if ["plugin", "ctx", "e", "sender", "label", "args", "server", "color"].contains(&name.as_str())
            || out.contains(&name)
        {
            name.push('_');
        }
        out.push(name);
        slot += if t == "J" || t == "D" { 2 } else { 1 };
    }
    out
}

/// A field of the plugin's struct, from a field of the main class.
#[derive(Clone, Debug)]
pub struct PluginField {
    pub rust: String,
    pub kind: Kind,
    pub ty: &'static str,
    /// Its value at construction, when the Java constructor sets a constant.
    pub init: Option<String>,
}

pub struct Spec<'a> {
    pub class: &'a Class,
    pub method: &'a Method,
    /// The main class's internal name.
    pub main: &'a str,
    pub fields: &'a BTreeMap<String, PluginField>,
    pub params: Vec<Param>,
    pub ret: Ret,
    pub has_config: bool,
    /// The command a `Command` parameter stands for.
    pub command: Option<&'a str>,
    /// The plugin's own methods it may call.
    pub helpers: Option<Helpers<'a>>,
}

/// A translated body: its lines (unindented) and which closure parameters it uses.
#[derive(Debug)]
pub struct Translated {
    pub lines: Vec<String>,
    pub uses: BTreeSet<&'static str>,
}

#[derive(Debug)]
enum Stmt {
    Line(String),
    If {
        cond: String,
        then: Vec<Stmt>,
        els: Vec<Stmt>,
    },
    Return(Option<String>),
    For {
        var: String,
        iter: String,
        body: Vec<Stmt>,
    },
    /// A switch: each arm's patterns and body, and the default's.
    Match {
        on: String,
        arms: Vec<(Vec<String>, Vec<Stmt>)>,
        default: Vec<Stmt>,
    },
}

/// Whether `stmts` never fall through their end (every path returns).
fn diverges(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Return(_)) => true,
        Some(Stmt::If { then, els, .. }) => !els.is_empty() && diverges(then) && diverges(els),
        Some(Stmt::Match { arms, default, .. }) => diverges(default) && arms.iter().all(|(_, b)| diverges(b)),
        _ => false,
    }
}

struct Local {
    name: String,
    kind: Kind,
}

struct Tr<'a> {
    spec: &'a Spec<'a>,
    body: &'a Body,
    /// Instruction start offsets.
    starts: BTreeSet<usize>,
    /// Slot → the parameter or local it holds.
    params: HashMap<u16, Expr>,
    locals: HashMap<u16, Local>,
    /// Slots stored to more than once (or `iinc`ed): `let mut`.
    reassigned: BTreeSet<u16>,
    names: BTreeSet<String>,
    /// The variable of the for-each loop being translated, once its body stored `next()`.
    loop_var: Option<String>,
    /// A string switch's first half was read: the index local's slot, the text switched on, and
    /// which strings each index stands for.
    string_switch: Option<(u16, Expr, BTreeMap<i32, Vec<String>>)>,
}

type R<T> = Result<T, String>;

pub fn translate(spec: &Spec) -> R<Translated> {
    let body = spec.method.body.as_ref().ok_or("no code")?;
    if body.handlers > 0 {
        return Err("it has try/catch (or synchronized)".into());
    }
    let mut starts = BTreeSet::new();
    let mut pc = 0;
    while pc < body.code.len() {
        starts.insert(pc);
        pc +=
            classfile::instruction_len(&body.code, pc).ok_or_else(|| format!("unknown opcode {:#x}", body.code[pc]))?;
    }
    let mut tr = Tr {
        spec,
        body,
        starts,
        params: HashMap::new(),
        locals: HashMap::new(),
        reassigned: BTreeSet::new(),
        names: ["plugin", "ctx", "e", "sender", "label", "args", "server", "color"]
            .into_iter()
            .map(String::from)
            .chain(spec.params.iter().filter_map(|p| match p {
                Param::Named(n, _) => Some(n.clone()),
                _ => None,
            }))
            .collect(),
        loop_var: None,
        string_switch: None,
    };
    tr.bind_params()?;
    tr.scan_stores();
    let mut stmts = Vec::new();
    tr.block(0, body.code.len(), &mut stmts)?;
    // A trailing return: `return;` goes, `return x;` becomes the closure's value.
    let mut lines = Vec::new();
    match stmts.last() {
        Some(Stmt::Return(None)) => {
            stmts.pop();
        }
        Some(Stmt::Return(Some(_))) => {}
        _ if spec.ret != Ret::Unit && !diverges(&stmts) => return Err("it doesn't end with a return".into()),
        _ => {}
    }
    let tail = match stmts.last() {
        Some(Stmt::Return(Some(v))) => {
            let v = v.clone();
            stmts.pop();
            Some(v)
        }
        _ => None,
    };
    render(&stmts, 0, &mut lines);
    if let Some(v) = tail {
        lines.push(v);
    }
    let mut uses = BTreeSet::new();
    for name in ["plugin", "ctx", "e", "sender", "label", "args"] {
        if lines.iter().any(|l| uses_ident(l, name)) {
            uses.insert(name);
        }
    }
    Ok(Translated { lines, uses })
}

/// Whether `line` names `ident` outside string literals.
pub fn uses_ident(line: &str, ident: &str) -> bool {
    let mut in_str = false;
    let mut escaped = false;
    let chars: Vec<char> = line.chars().collect();
    let id: Vec<char> = ident.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            i += 1;
            continue;
        }
        let word = |c: char| c.is_alphanumeric() || c == '_';
        if chars[i..].starts_with(&id)
            && (i == 0 || !word(chars[i - 1]))
            && chars.get(i + id.len()).is_none_or(|c| !word(*c))
        {
            return true;
        }
        i += 1;
    }
    false
}

fn render(stmts: &[Stmt], depth: usize, out: &mut Vec<String>) {
    let pad = "    ".repeat(depth);
    for s in stmts {
        match s {
            Stmt::Line(l) => out.push(format!("{pad}{l}")),
            Stmt::Return(None) => out.push(format!("{pad}return;")),
            Stmt::Return(Some(v)) => out.push(format!("{pad}return {v};")),
            Stmt::For { var, iter, body } => {
                out.push(format!("{pad}for {var} in {iter} {{"));
                render(body, depth + 1, out);
                out.push(format!("{pad}}}"));
            }
            Stmt::Match { on, arms, default } => {
                out.push(format!("{pad}match {on} {{"));
                for (patterns, body) in arms {
                    out.push(format!("{pad}    {} => {{", patterns.join(" | ")));
                    render(body, depth + 2, out);
                    out.push(format!("{pad}    }}"));
                }
                if default.is_empty() {
                    out.push(format!("{pad}    _ => {{}}"));
                } else {
                    out.push(format!("{pad}    _ => {{"));
                    render(default, depth + 2, out);
                    out.push(format!("{pad}    }}"));
                }
                out.push(format!("{pad}}}"));
            }
            Stmt::If { cond, then, els } => {
                // `if a { if b { x } }` → `if a && b { x }`
                if let ([Stmt::If { cond: inner, then: t2, els: e2 }], true) = (then.as_slice(), els.is_empty()) {
                    if e2.is_empty() {
                        let merged = Stmt::If {
                            cond: format!("{} && {}", and_part(cond), and_part(inner)),
                            then: steal(t2),
                            els: Vec::new(),
                        };
                        render(std::slice::from_ref(&merged), depth, out);
                        continue;
                    }
                }
                out.push(format!("{pad}if {cond} {{"));
                render(then, depth + 1, out);
                if els.is_empty() {
                    out.push(format!("{pad}}}"));
                } else if let [Stmt::If { .. }] = els.as_slice() {
                    let mut inner = Vec::new();
                    render(els, depth, &mut inner);
                    let first = inner.remove(0);
                    out.push(format!("{pad}}} else {}", first.trim_start()));
                    out.extend(inner);
                } else {
                    out.push(format!("{pad}}} else {{"));
                    render(els, depth + 1, out);
                    out.push(format!("{pad}}}"));
                }
            }
        }
    }
}

/// A condition as one side of `&&`.
fn and_part(cond: &str) -> String {
    if cond.contains("||") { format!("({cond})") } else { cond.to_owned() }
}

fn steal(stmts: &[Stmt]) -> Vec<Stmt> {
    stmts
        .iter()
        .map(|s| match s {
            Stmt::Line(l) => Stmt::Line(l.clone()),
            Stmt::Return(v) => Stmt::Return(v.clone()),
            Stmt::If { cond, then, els } => Stmt::If { cond: cond.clone(), then: steal(then), els: steal(els) },
            Stmt::For { var, iter, body } => Stmt::For { var: var.clone(), iter: iter.clone(), body: steal(body) },
            Stmt::Match { on, arms, default } => Stmt::Match {
                on: on.clone(),
                arms: arms.iter().map(|(p, b)| (p.clone(), steal(b))).collect(),
                default: steal(default),
            },
        })
        .collect()
}

/// `joinCount` → `join_count`, kept clear of Rust's keywords and the closure's own names.
pub fn snake(java: &str) -> String {
    let mut out = String::new();
    for (i, c) in java.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'v');
    }
    const KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl",
        "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct",
        "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract",
        "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
        "gen",
    ];
    if KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// A Java field or local's descriptor as a Rust kind and type, for the simple ones.
pub fn field_kind(descriptor: &str) -> Option<(Kind, &'static str)> {
    Some(match descriptor {
        "I" | "S" | "B" => (Kind::I32, "i32"),
        "J" => (Kind::I64, "i64"),
        "D" => (Kind::F64, "f64"),
        "F" => (Kind::F32, "f32"),
        "Z" => (Kind::Bool, "bool"),
        "C" => (Kind::Char, "char"),
        "Ljava/lang/String;" => (Kind::Str, "String"),
        _ => return None,
    })
}

/// The plugin's struct fields: the main class's fields of simple types, with the constants its
/// constructor sets them to.
pub fn plugin_fields(main: &Class) -> BTreeMap<String, PluginField> {
    let mut out = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for f in main.fields.iter().filter(|f| !f.is_static) {
        let Some((kind, ty)) = field_kind(&f.descriptor) else { continue };
        let mut rust = snake(&f.name);
        while !taken.insert(rust.clone()) {
            rust.push('_');
        }
        out.insert(f.name.clone(), PluginField { rust, kind, ty, init: None });
    }
    // `this.x = <constant>` in the constructor.
    if let Some(body) = main.method("<init>").and_then(|m| m.body.as_ref()) {
        let code = &body.code;
        let mut pc = 0;
        let mut last: Option<String> = None;
        while pc < code.len() {
            let Some(n) = classfile::instruction_len(code, pc) else { break };
            let op = code[pc];
            let u16_at = |p: usize| u16::from_be_bytes([code[p + 1], code[p + 2]]);
            last = match op {
                0x02..=0x08 => Some((op as i64 - 3).to_string()),
                0x09 | 0x0a => Some((op as i64 - 9).to_string()),
                0x0b..=0x0d => Some(format!("{:?}", (op - 0x0b) as f32)),
                0x0e | 0x0f => Some(format!("{:?}", (op - 0x0e) as f64)),
                0x10 => Some((code[pc + 1] as i8).to_string()),
                0x11 => Some((u16_at(pc) as i16).to_string()),
                0x12..=0x14 => {
                    let i = if op == 0x12 { code[pc + 1] as u16 } else { u16_at(pc) };
                    match main.constant(i) {
                        Some(Const::Int(v)) => Some(v.to_string()),
                        Some(Const::Long(v)) => Some(v.to_string()),
                        Some(Const::Float(v)) => Some(format!("{v:?}")),
                        Some(Const::Double(v)) => Some(format!("{v:?}")),
                        Some(Const::Str(s)) => Some(format!("{s:?}.to_owned()")),
                        _ => None,
                    }
                }
                0xb5 => {
                    if let (Some(v), Some(m)) = (last.take(), main.member(u16_at(pc))) {
                        if m.owner == main.name {
                            if let Some(f) = out.get_mut(&m.name) {
                                f.init = Some(match (&f.kind, v.as_str()) {
                                    (Kind::Bool, "0") => "false".into(),
                                    (Kind::Bool, "1") => "true".into(),
                                    (Kind::Char, c) => c
                                        .parse::<u32>()
                                        .ok()
                                        .and_then(char::from_u32)
                                        .map_or(v.clone(), |c| format!("{c:?}")),
                                    _ => v,
                                });
                            }
                        }
                    }
                    None
                }
                _ => None,
            };
            pc += n;
        }
    }
    out
}

fn ret_letter(descriptor: &str) -> &str {
    descriptor.rsplit(')').next().unwrap_or("V")
}

impl<'a> Tr<'a> {
    fn code(&self) -> &[u8] {
        &self.body.code
    }
    fn u16_at(&self, pc: usize) -> u16 {
        u16::from_be_bytes([self.code()[pc + 1], self.code()[pc + 2]])
    }
    fn i16_at(&self, pc: usize) -> i16 {
        self.u16_at(pc) as i16
    }

    fn bind_params(&mut self) -> R<()> {
        let m = self.spec.method;
        let mut slot = 0u16;
        if !m.is_static {
            let this = if self.spec.class.name == self.spec.main {
                Expr::new("plugin", Kind::Plugin)
            } else {
                Expr::new("self", Kind::Listener)
            };
            self.params.insert(0, this);
            slot = 1;
        }
        let types = classfile::parameters(&m.descriptor);
        if types.len() != self.spec.params.len() {
            return Err(format!("it takes {} parameters", types.len()));
        }
        for (p, ty) in self.spec.params.iter().zip(&types) {
            let e = match p {
                Param::Event(name) => Expr::new("e", Kind::Event(name)),
                Param::Sender => Expr::new("sender", Kind::Sender),
                Param::Command => Expr::new("command", Kind::Command),
                Param::Label => Expr::new("label", Kind::StrRef),
                Param::Args => Expr::new("args", Kind::Args),
                Param::Named(name, kind) => Expr::new(name.clone(), kind.clone()),
            };
            self.params.insert(slot, e);
            slot += if ty == "J" || ty == "D" { 2 } else { 1 };
        }
        Ok(())
    }

    fn scan_stores(&mut self) {
        let mut count: HashMap<u16, u32> = HashMap::new();
        for &pc in &self.starts {
            let op = self.code()[pc];
            let slot = match op {
                0x36..=0x3a => Some(self.code()[pc + 1] as u16),
                0x3b..=0x4e => Some(((op - 0x3b) % 4) as u16),
                0x84 => {
                    self.reassigned.insert(self.code()[pc + 1] as u16);
                    None
                }
                _ => None,
            };
            if let Some(s) = slot {
                *count.entry(s).or_default() += 1;
            }
        }
        self.reassigned.extend(count.into_iter().filter(|(_, n)| *n > 1).map(|(s, _)| s));
    }

    /// The instruction that ends right at `pc`.
    fn before(&self, pc: usize) -> Option<usize> {
        self.starts.range(..pc).next_back().copied()
    }

    fn block(&mut self, start: usize, end: usize, out: &mut Vec<Stmt>) -> R<()> {
        let mut stack: Vec<Expr> = Vec::new();
        let mut pc = start;
        while pc < end {
            let op = self.code()[pc];
            let len = classfile::instruction_len(self.code(), pc).unwrap_or(1);
            match op {
                0x99 if matches!(stack.last().map(|e| &e.kind), Some(Kind::HasNext(..))) => {
                    // for (X x : collection) { body }: `ifeq END` here, `goto` back before END.
                    let Some(Expr { kind: Kind::HasNext(iter, _), .. }) = stack.pop() else { unreachable!() };
                    let target = (pc as i64 + self.i16_at(pc) as i64) as usize;
                    let back =
                        self.before(target).filter(|&g| self.code()[g] == 0xa7).ok_or("a loop without a goto back")?;
                    let dest = (back as i64 + self.i16_at(back) as i64) as usize;
                    if dest >= pc || target > end || !stack.is_empty() {
                        return Err("a loop of an unusual shape".into());
                    }
                    let outer = self.loop_var.take();
                    let mut body = Vec::new();
                    self.block(pc + len, back, &mut body)?;
                    let var = std::mem::replace(&mut self.loop_var, outer).ok_or("a loop whose item isn't kept")?;
                    out.push(Stmt::For { var, iter, body });
                    pc = target;
                    continue;
                }
                0xaa | 0xab => {
                    let key = self.pop(&mut stack)?;
                    if !stack.is_empty() {
                        return Err("a switch with values on the stack".into());
                    }
                    let (default, cases) = self.switch_table(pc).ok_or("a malformed switch")?;
                    if let Kind::Hash(text) = &key.kind {
                        pc = self.string_switch_first_half(pc + len, default, (**text).clone(), out)?;
                        continue;
                    }
                    pc = self.switch(pc, key, default, cases, end, out)?;
                    continue;
                }
                0x99..=0xa6 | 0xc6 | 0xc7 => {
                    let target = (pc as i64 + self.i16_at(pc) as i64) as usize;
                    if target <= pc {
                        return Err("it has a loop".into());
                    }
                    if target > end {
                        return Err("its branches don't nest (|| or a jump out)".into());
                    }
                    let cond = self.then_condition(op, &mut stack)?;
                    if !stack.is_empty() {
                        return Err("a value crosses a branch (a ?: expression)".into());
                    }
                    let then_start = pc + len;
                    let (then_end, els, next) = match self.before(target) {
                        Some(g) if g >= then_start && self.code()[g] == 0xa7 => {
                            let dest = (g as i64 + self.i16_at(g) as i64) as usize;
                            if dest > target && dest <= end {
                                (g, Some((target, dest)), dest)
                            } else if dest == target {
                                (g, None, target)
                            } else {
                                return Err("its branches don't nest".into());
                            }
                        }
                        _ => (target, None, target),
                    };
                    let mut then = Vec::new();
                    self.block(then_start, then_end, &mut then)?;
                    let mut els_stmts = Vec::new();
                    if let Some((a, b)) = els {
                        self.block(a, b, &mut els_stmts)?;
                    }
                    out.push(Stmt::If { cond, then, els: els_stmts });
                    pc = next;
                    continue;
                }
                0xa7 => return Err("it has a goto that isn't an if/else".into()),
                0xac..=0xb0 => {
                    let v = stack.pop().ok_or("return with an empty stack")?;
                    let v = match self.spec.ret {
                        Ret::Bool => self.boolean(&v)?,
                        Ret::StrList => {
                            if v.kind != Kind::StrList {
                                return Err("it returns something other than a list of strings".into());
                            }
                            v.code
                        }
                        Ret::Value(k) => match k {
                            ValueKind::Str => self.s_owned(&v)?,
                            ValueKind::I32 => self.cast(v, Kind::I32).code,
                            ValueKind::I64 => self.cast(v, Kind::I64).code,
                            ValueKind::F64 => self.cast(v, Kind::F64).code,
                        },
                        Ret::Unit => return Err("it returns a value".into()),
                    };
                    out.push(Stmt::Return(Some(v)));
                }
                0xb1 => {
                    if self.spec.ret != Ret::Unit {
                        return Err("it returns nothing".into());
                    }
                    out.push(Stmt::Return(None));
                }
                _ => self.step(pc, op, &mut stack, out)?,
            }
            pc += len;
        }
        if !stack.is_empty() {
            return Err("a value is left at a branch's end (a ?: expression)".into());
        }
        Ok(())
    }

    /// A tableswitch or lookupswitch at `pc`: its default target and (key, target) pairs.
    fn switch_table(&self, pc: usize) -> Option<(usize, Vec<(i32, usize)>)> {
        let code = self.code();
        let base = (pc + 4) & !3;
        let int = |at: usize| code.get(at..at + 4).map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        let to = |off: i32| (pc as i64 + off as i64) as usize;
        let default = to(int(base)?);
        let mut cases = Vec::new();
        if code[pc] == 0xaa {
            let (lo, hi) = (int(base + 4)?, int(base + 8)?);
            for (i, k) in (lo..=hi).enumerate() {
                cases.push((k, to(int(base + 12 + 4 * i)?)));
            }
        } else {
            let n = int(base + 4)?;
            for i in 0..n.max(0) as usize {
                cases.push((int(base + 8 + 8 * i)?, to(int(base + 12 + 8 * i)?)));
            }
        }
        Some((default, cases))
    }

    /// The first half of a string switch (`switch (text.hashCode())`, each case comparing with
    /// `equals` and setting an index): reads which strings set which index, and goes on at the second
    /// half, the switch on the index.
    fn string_switch_first_half(&mut self, from: usize, to: usize, text: Expr, out: &mut Vec<Stmt>) -> R<usize> {
        let mut names: BTreeMap<i32, Vec<String>> = BTreeMap::new();
        let mut slot = None;
        let mut current: Option<String> = None;
        let mut int: Option<i32> = None;
        for &pc in self.starts.range(from..to) {
            let op = self.code()[pc];
            match op {
                0x12 | 0x13 => {
                    let i = if op == 0x12 { self.code()[pc + 1] as u16 } else { self.u16_at(pc) };
                    if let Some(Const::Str(s)) = self.spec.class.constant(i) {
                        current = Some(s);
                    }
                }
                0x02..=0x08 => int = Some(op as i32 - 3),
                0x10 => int = Some(self.code()[pc + 1] as i8 as i32),
                0x11 => int = Some(self.i16_at(pc) as i32),
                0x36 | 0x3b..=0x3e => {
                    let s = if op == 0x36 { self.code()[pc + 1] as u16 } else { (op - 0x3b) as u16 };
                    let (Some(k), Some(name)) = (int.take(), current.take()) else {
                        return Err("a string switch of an unusual shape".into());
                    };
                    if slot.is_some_and(|x| x != s) {
                        return Err("a string switch of an unusual shape".into());
                    }
                    slot = Some(s);
                    names.entry(k).or_default().push(name);
                }
                _ => {}
            }
        }
        let slot = slot.ok_or("a string switch with no cases")?;
        // javac keeps the text in a variable of its own (no name in the class file): the match reads
        // the expression itself.
        let mut text = text;
        let synthetic =
            text.code.strip_prefix('v').is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
        if synthetic {
            let decl = format!("let {} = ", text.code);
            if let Some(i) = out.iter().rposition(|s| matches!(s, Stmt::Line(l) if l.starts_with(&decl))) {
                if let Stmt::Line(l) = out.remove(i) {
                    let value = l[decl.len()..].trim_end_matches(';').to_owned();
                    let value = value.strip_suffix(".clone()").map(str::to_owned).unwrap_or(value);
                    text = Expr { code: value, kind: text.kind, atom: true, place: false, lit: None };
                }
            }
        }
        // `int index = -1;` before it is the switch's own: it goes.
        if let Some(l) = self.locals.get(&slot) {
            let decl = format!("let mut {} = ", l.name);
            out.retain(|s| !matches!(s, Stmt::Line(line) if line.starts_with(&decl)));
        }
        self.string_switch = Some((slot, text, names));
        Ok(to)
    }

    /// A switch whose cases end in `break` (a goto past them all) or `return`.
    fn switch(
        &mut self,
        pc: usize,
        key: Expr,
        default: usize,
        cases: Vec<(i32, usize)>,
        end: usize,
        out: &mut Vec<Stmt>,
    ) -> R<usize> {
        // The text a string switch's index stands for.
        let strings = match self.string_switch.take() {
            Some((slot, text, names)) if self.locals.get(&slot).is_some_and(|l| l.name == key.code) => {
                Some((text, names))
            }
            other => {
                self.string_switch = other;
                None
            }
        };
        let mut targets: Vec<usize> = cases.iter().map(|c| c.1).chain([default]).collect();
        targets.sort_unstable();
        targets.dedup();
        if targets.first().is_some_and(|&t| t <= pc) {
            return Err("a switch that jumps back".into());
        }
        // Where the cases' breaks go.
        let mut finish = None;
        for w in targets.windows(2) {
            if let Some(g) = self.before(w[1]).filter(|&g| g >= w[0] && self.code()[g] == 0xa7) {
                let dest = (g as i64 + self.i16_at(g) as i64) as usize;
                if dest > *targets.last().unwrap() {
                    finish = Some(finish.map_or(dest, |f: usize| f.max(dest)));
                }
            }
        }
        let finish = finish.unwrap_or(end).min(end);
        let mut arms = Vec::new();
        let mut default_body = Vec::new();
        for (i, &t) in targets.iter().enumerate() {
            if t >= finish {
                continue;
            }
            let next = targets.get(i + 1).copied().unwrap_or(finish).min(finish);
            let last = self.before(next).ok_or("an empty case")?;
            let body_end = match self.code()[last] {
                0xa7 => {
                    let dest = (last as i64 + self.i16_at(last) as i64) as usize;
                    if dest != finish {
                        return Err("a case that jumps elsewhere".into());
                    }
                    last
                }
                0xac..=0xb1 | 0xbf => next,
                _ if next == finish => next,
                _ => return Err("a case that falls through".into()),
            };
            let mut body = Vec::new();
            self.block(t, body_end, &mut body)?;
            if t == default {
                default_body = body;
                continue;
            }
            let mut patterns = Vec::new();
            for (k, _) in cases.iter().filter(|c| c.1 == t) {
                match &strings {
                    Some((_, names)) => {
                        for n in names.get(k).ok_or("a string switch index with no string")? {
                            patterns.push(format!("{n:?}"));
                        }
                    }
                    None => patterns.push(k.to_string()),
                }
            }
            arms.push((patterns, body));
        }
        let on = match &strings {
            Some((text, _)) => match text.kind {
                Kind::Str => format!("{}.as_str()", text.recv()),
                Kind::StrRef => text.code.clone(),
                _ => return Err("a switch on something other than text".into()),
            },
            None if matches!(key.kind, Kind::I32 | Kind::Char) => key.code.clone(),
            None => return Err(format!("a switch on {:?}", key.kind)),
        };
        out.push(Stmt::Match { on, arms, default: default_body });
        Ok(finish)
    }

    fn pop(&self, stack: &mut Vec<Expr>) -> R<Expr> {
        stack.pop().ok_or_else(|| "the stack runs dry".into())
    }

    /// The condition under which a branch at `op` is not taken (the `if` body runs).
    fn then_condition(&self, op: u8, stack: &mut Vec<Expr>) -> R<String> {
        let rel = |op: u8| match op {
            0 => "==",
            1 => "!=",
            2 => "<",
            3 => ">=",
            4 => ">",
            _ => "<=",
        };
        // The inverse of each comparison: the body runs when the jump's isn't true.
        let inv = |op: u8| match op {
            0 => 1,
            1 => 0,
            2 => 3,
            3 => 2,
            4 => 5,
            _ => 4,
        };
        Ok(match op {
            0x99..=0x9e => {
                let v = self.pop(stack)?;
                let k = op - 0x99;
                match &v.kind {
                    Kind::Bool if k <= 1 => {
                        if k == 0 {
                            v.code
                        } else {
                            format!("!{}", v.recv())
                        }
                    }
                    Kind::Compare(a, b) => format!("{} {} {}", a.recv(), rel(inv(k)), b.recv()),
                    Kind::I32 | Kind::I64 => Self::compare(&v, rel(inv(k)), &Expr::int(0, Kind::I32)),
                    _ => return Err(format!("a branch on {:?}", v.kind)),
                }
            }
            0x9f..=0xa4 => {
                let b = self.pop(stack)?;
                let a = self.pop(stack)?;
                let (a, b) = self.same_number(a, b)?;
                Self::compare(&a, rel(inv(op - 0x9f)), &b)
            }
            0xa5 | 0xa6 => {
                let b = self.pop(stack)?;
                let a = self.pop(stack)?;
                let eq = |a: &Expr, b: &Expr| -> R<String> {
                    let comparable = |k: &Kind| matches!(k, Kind::Str | Kind::StrRef);
                    if (comparable(&a.kind) && comparable(&b.kind))
                        || (a.kind == b.kind && matches!(a.kind, Kind::GameMode | Kind::Color))
                    {
                        Ok(format!("{} == {}", a.recv(), b.recv()))
                    } else {
                        Err(format!("comparing {:?} and {:?}", a.kind, b.kind))
                    }
                };
                let e = eq(&a, &b)?;
                if op == 0xa5 { e.replacen(" == ", " != ", 1) } else { e }
            }
            _ => {
                let v = self.pop(stack)?;
                let _ = v;
                return Err("a null check".into());
            }
        })
    }

    /// `a op b`, with `x.len() as i32` compared to a count as `x.len()` (`is_empty()` against 0).
    fn compare(a: &Expr, op: &str, b: &Expr) -> String {
        if let (Some(len), Some(Lit::Int(n))) = (a.code.strip_suffix(".len() as i32"), &b.lit) {
            if *n == 0 {
                match op {
                    "==" | "<=" => return format!("{len}.is_empty()"),
                    "!=" | ">" => return format!("!{len}.is_empty()"),
                    _ => {}
                }
            }
            if *n >= 0 {
                return format!("{len}.len() {op} {n}");
            }
        }
        format!("{} {op} {}", a.recv(), b.recv())
    }

    fn same_number(&self, a: Expr, b: Expr) -> R<(Expr, Expr)> {
        let num = |k: &Kind| matches!(k, Kind::I32 | Kind::I64 | Kind::F32 | Kind::F64 | Kind::Char);
        if !num(&a.kind) || !num(&b.kind) {
            return Err(format!("arithmetic on {:?} and {:?}", a.kind, b.kind));
        }
        if a.kind == b.kind {
            return Ok((a, b));
        }
        // A literal takes the other side's type.
        if b.lit.is_some() {
            let k = a.kind.clone();
            return Ok((a, self.cast(b, k)));
        }
        if a.lit.is_some() {
            let k = b.kind.clone();
            return Ok((self.cast(a, k), b));
        }
        Err(format!("mixing {:?} and {:?}", a.kind, b.kind))
    }

    fn rust_ty(k: &Kind) -> &'static str {
        match k {
            Kind::I32 => "i32",
            Kind::I64 => "i64",
            Kind::F32 => "f32",
            Kind::F64 => "f64",
            Kind::Char => "char",
            _ => "_",
        }
    }

    fn cast(&self, e: Expr, to: Kind) -> Expr {
        if e.kind == to {
            return e;
        }
        match (&e.lit, &to) {
            (Some(Lit::Int(v)), Kind::I32 | Kind::I64) => return Expr::int(*v, to),
            (Some(Lit::Int(v)), Kind::F32 | Kind::F64) => return Expr::float(*v as f64, to),
            (Some(Lit::Float(v)), Kind::F32 | Kind::F64) => return Expr::float(*v, to),
            (Some(Lit::Int(v)), Kind::Char) => {
                if let Some(c) = char::from_u32(*v as u32) {
                    return Expr { code: format!("{c:?}"), kind: Kind::Char, atom: true, place: false, lit: e.lit };
                }
            }
            _ => {}
        }
        Expr::op(format!("{} as {}", e.recv(), Self::rust_ty(&to)), to)
    }

    /// `e` as a `bool`.
    fn boolean(&self, e: &Expr) -> R<String> {
        match (&e.kind, &e.lit) {
            (Kind::Bool, _) => Ok(e.code.clone()),
            (Kind::I32, Some(Lit::Int(0))) => Ok("false".into()),
            (Kind::I32, Some(Lit::Int(1))) => Ok("true".into()),
            (Kind::I32, _) => Ok(format!("{} != 0", e.recv())),
            (k, _) => Err(format!("{k:?} as a boolean")),
        }
    }

    /// `e` where Rust wants `&str`.
    fn s_ref(&self, e: &Expr) -> R<String> {
        match e.kind {
            Kind::StrRef => Ok(e.code.clone()),
            Kind::Str => Ok(format!("&{}", e.code)),
            _ => Err(format!("{:?} as text", e.kind)),
        }
    }

    /// `e` where Rust wants a `String`.
    fn s_owned(&self, e: &Expr) -> R<String> {
        match e.kind {
            Kind::Str if e.place => Ok(format!("{}.clone()", e.recv())),
            Kind::Str => Ok(e.code.clone()),
            Kind::StrRef => Ok(format!("{}.to_owned()", e.recv())),
            _ => Err(format!("{:?} as text", e.kind)),
        }
    }

    /// `e` converted to the Java parameter type `ty` (a descriptor letter or class).
    fn arg(&self, e: Expr, ty: &str) -> R<Expr> {
        Ok(match ty {
            "Z" => Expr::new(self.boolean(&e)?, Kind::Bool),
            "C" => self.cast(e, Kind::Char),
            "I" | "S" | "B" => self.cast(e, Kind::I32),
            "J" => self.cast(e, Kind::I64),
            "F" => self.cast(e, Kind::F32),
            "D" => self.cast(e, Kind::F64),
            _ => e,
        })
    }

    fn local_name(&mut self, slot: u16, pc: usize) -> String {
        // The variable whose scope starts right after this store; else any in this slot.
        let mut in_slot: Vec<_> = self.body.locals.iter().filter(|l| l.slot == slot).collect();
        in_slot.sort_by_key(|l| l.start);
        let java = in_slot
            .iter()
            .find(|l| l.start as usize > pc)
            .or_else(|| in_slot.iter().rev().find(|l| l.start as usize <= pc))
            .map(|l| l.name.clone());
        let mut name = java.map(|n| snake(&n)).unwrap_or_else(|| format!("v{slot}"));
        while self.names.contains(&name) {
            name.push('_');
        }
        self.names.insert(name.clone());
        name
    }

    fn load(&self, slot: u16) -> R<Expr> {
        if let Some(p) = self.params.get(&slot) {
            return Ok(p.clone());
        }
        let l = self.locals.get(&slot).ok_or("a local read before it is set")?;
        if matches!(l.kind, Kind::Iter(..)) {
            return Ok(Expr::new("", l.kind.clone()));
        }
        let mut e = Expr::new(l.name.clone(), l.kind.clone());
        e.place = !l.name.contains('(');
        Ok(e)
    }

    fn store(&mut self, slot: u16, pc: usize, v: Expr, out: &mut Vec<Stmt>) -> R<()> {
        if self.params.contains_key(&slot) {
            return Err("it assigns to a parameter".into());
        }
        if matches!(
            v.kind,
            Kind::Null
                | Kind::New(_)
                | Kind::Builder(_)
                | Kind::Array(_)
                | Kind::Compare(..)
                | Kind::Ignored
                | Kind::Listener
                | Kind::Command
                | Kind::Hash(_)
                | Kind::HasNext(..)
        ) {
            return Err(format!("a local holding {:?}", v.kind));
        }
        if let Kind::LoopItem(item) = &v.kind {
            if self.locals.contains_key(&slot) || self.loop_var.is_some() {
                return Err("a loop variable kept twice".into());
            }
            let name = self.local_name(slot, pc);
            self.locals.insert(slot, Local { name: name.clone(), kind: (**item).clone() });
            self.loop_var = Some(name);
            return Ok(());
        }
        if matches!(v.kind, Kind::Iter(..)) {
            if self.locals.contains_key(&slot) {
                return Err("an iterator kept twice".into());
            }
            self.locals.insert(slot, Local { name: String::new(), kind: v.kind });
            return Ok(());
        }
        // What only stands for a handle (ctx.config(), server()) isn't kept in a local: the local
        // becomes another name for it, so `ctx` isn't held borrowed.
        if matches!(v.kind, Kind::Config | Kind::Logger | Kind::Server | Kind::Description | Kind::Plugin)
            && !v.code.contains("((")
        {
            if self.locals.contains_key(&slot) {
                return Err("a local changes".into());
            }
            self.locals.insert(slot, Local { name: v.code.clone(), kind: v.kind });
            return Ok(());
        }
        let value = match v.kind {
            Kind::Str if v.place => format!("{}.clone()", v.recv()),
            Kind::Player | Kind::Location | Kind::Block | Kind::BlockPos | Kind::World if v.place => {
                format!("{}.clone()", v.recv())
            }
            _ => v.code.clone(),
        };
        if let Some(l) = self.locals.get(&slot) {
            if l.kind != v.kind {
                return Err("a local changes type".into());
            }
            out.push(Stmt::Line(format!("{} = {value};", l.name)));
        } else {
            let name = self.local_name(slot, pc);
            let m = if self.reassigned.contains(&slot) { "mut " } else { "" };
            out.push(Stmt::Line(format!("let {m}{name} = {value};")));
            self.locals.insert(slot, Local { name, kind: v.kind });
        }
        Ok(())
    }

    fn step(&mut self, pc: usize, op: u8, stack: &mut Vec<Expr>, out: &mut Vec<Stmt>) -> R<()> {
        match op {
            0x00 => {}
            0x01 => stack.push(Expr::new("None", Kind::Null)),
            0x02..=0x08 => stack.push(Expr::int(op as i64 - 3, Kind::I32)),
            0x09 | 0x0a => stack.push(Expr::int(op as i64 - 9, Kind::I64)),
            0x0b..=0x0d => stack.push(Expr::float((op - 0x0b) as f64, Kind::F32)),
            0x0e | 0x0f => stack.push(Expr::float((op - 0x0e) as f64, Kind::F64)),
            0x10 => stack.push(Expr::int(self.code()[pc + 1] as i8 as i64, Kind::I32)),
            0x11 => stack.push(Expr::int(self.i16_at(pc) as i64, Kind::I32)),
            0x12..=0x14 => {
                let i = if op == 0x12 { self.code()[pc + 1] as u16 } else { self.u16_at(pc) };
                stack.push(match self.spec.class.constant(i) {
                    Some(Const::Int(v)) => Expr::int(v as i64, Kind::I32),
                    Some(Const::Long(v)) => Expr::int(v, Kind::I64),
                    Some(Const::Float(v)) => Expr::float(v as f64, Kind::F32),
                    Some(Const::Double(v)) => Expr::float(v, Kind::F64),
                    Some(Const::Str(s)) => Expr::string(&s),
                    _ => return Err("a class literal".into()),
                });
            }
            0x15..=0x19 => stack.push(self.load(self.code()[pc + 1] as u16)?),
            0x1a..=0x2d => stack.push(self.load(((op - 0x1a) % 4) as u16)?),
            0x32 => {
                let i = self.pop(stack)?;
                let a = self.pop(stack)?;
                if a.kind != Kind::Args {
                    return Err("an array other than a command's arguments".into());
                }
                let idx = match i.lit {
                    Some(Lit::Int(v)) => v.to_string(),
                    _ => format!("{} as usize", i.recv()),
                };
                stack.push(Expr::place(format!("{}[{idx}]", a.recv()), Kind::Str));
            }
            0x36..=0x3a => {
                let v = self.pop(stack)?;
                self.store(self.code()[pc + 1] as u16, pc, v, out)?;
            }
            0x3b..=0x4e => {
                let v = self.pop(stack)?;
                self.store(((op - 0x3b) % 4) as u16, pc, v, out)?;
            }
            0x53 => {
                let v = self.pop(stack)?;
                let i = self.pop(stack)?;
                let mut a = self.pop(stack)?;
                let Kind::Array(items) = &mut a.kind else { return Err("storing into an array".into()) };
                let Some(Lit::Int(i)) = i.lit else { return Err("an array index that isn't a constant".into()) };
                let slot = items.get_mut(i as usize).ok_or("an array index out of range")?;
                *slot = Some(v);
                // The array was dup'ed: the copy below is the one that stays.
                if let Some(top) = stack.last_mut() {
                    if matches!(top.kind, Kind::Array(_)) {
                        *top = a;
                    }
                }
            }
            0x57 => {
                let v = self.pop(stack)?;
                // A call's result nobody wants: the call stays. A value read for nothing goes.
                if v.kind != Kind::Ignored && v.code.contains('(') {
                    out.push(Stmt::Line(format!("{};", v.code)));
                }
            }
            0x58 => {
                let v = self.pop(stack)?;
                if !matches!(v.kind, Kind::I64 | Kind::F64) {
                    return Err("pop2 of two values".into());
                }
            }
            0x59 => {
                let v = stack.last().cloned().ok_or("dup on an empty stack")?;
                stack.push(v);
            }
            0x60..=0x73 => {
                let b = self.pop(stack)?;
                let a = self.pop(stack)?;
                let (a, b) = self.same_number(a, b)?;
                let sym = ["+", "-", "*", "/", "%"][((op - 0x60) / 4) as usize];
                let k = a.kind.clone();
                stack.push(Expr::op(format!("{} {sym} {}", a.recv(), b.recv()), k));
            }
            0x74..=0x77 => {
                let a = self.pop(stack)?;
                let k = a.kind.clone();
                stack.push(Expr::op(format!("-{}", a.recv()), k));
            }
            0x84 => {
                let slot = self.code()[pc + 1] as u16;
                let by = self.code()[pc + 2] as i8;
                let l = self.locals.get(&slot).ok_or("iinc of an unset local")?;
                out.push(Stmt::Line(if by >= 0 {
                    format!("{} += {by};", l.name)
                } else {
                    format!("{} -= {};", l.name, -(by as i32))
                }));
            }
            0x85..=0x93 => {
                let a = self.pop(stack)?;
                let to = match op {
                    0x85 | 0x8c | 0x8f => Kind::I64,
                    0x86 | 0x89 | 0x90 => Kind::F32,
                    0x87 | 0x8a | 0x8d => Kind::F64,
                    0x88 | 0x8b | 0x8e => Kind::I32,
                    0x92 => Kind::Char,
                    _ => Kind::I32,
                };
                stack.push(self.cast(a, to));
            }
            0x94..=0x98 => {
                let b = self.pop(stack)?;
                let a = self.pop(stack)?;
                let (a, b) = self.same_number(a, b)?;
                stack.push(Expr::new(String::new(), Kind::Compare(Box::new(a), Box::new(b))));
            }
            0xb2 => {
                let m = self.spec.class.member(self.u16_at(pc)).ok_or("a bad field reference")?;
                stack.push(static_field(&m)?);
            }
            0xb4 => {
                let m = self.spec.class.member(self.u16_at(pc)).ok_or("a bad field reference")?;
                let obj = self.pop(stack)?;
                stack.push(self.get_field(&obj, &m)?);
            }
            0xb5 => {
                let m = self.spec.class.member(self.u16_at(pc)).ok_or("a bad field reference")?;
                let v = self.pop(stack)?;
                let obj = self.pop(stack)?;
                let f = match obj.kind {
                    Kind::Plugin if m.owner == self.spec.main => {
                        self.spec.fields.get(&m.name).ok_or("a field of the plugin with no Rust type")?
                    }
                    _ => return Err("setting a field of something other than the plugin".into()),
                };
                let v = match f.kind {
                    Kind::Bool => self.boolean(&v)?,
                    Kind::Str => self.s_owned(&v)?,
                    _ => self.cast(v, f.kind.clone()).code,
                };
                // `x = x + 1` → `x += 1`
                let field = format!("plugin.{}", f.rust);
                let line = [" + ", " - ", " * "]
                    .iter()
                    .find_map(|op| {
                        v.strip_prefix(&format!("{field}{op}")).map(|rest| format!("{field} {}= {rest};", op.trim()))
                    })
                    .unwrap_or_else(|| format!("{field} = {v};"));
                out.push(Stmt::Line(line));
            }
            0xb6..=0xb9 => {
                let m = self.spec.class.member(self.u16_at(pc)).ok_or("a bad method reference")?;
                let types = classfile::parameters(&m.descriptor);
                let mut args = Vec::new();
                for _ in 0..types.len() {
                    args.push(self.pop(stack)?);
                }
                args.reverse();
                let args = args.into_iter().zip(&types).map(|(a, t)| self.arg(a, t)).collect::<R<Vec<_>>>()?;
                let recv = if op == 0xb8 { None } else { Some(self.pop(stack)?) };
                if op == 0xb7 && m.name == "<init>" {
                    let recv = recv.ok_or("a constructor without an object")?;
                    let made = construct(&m, recv, args, self)?;
                    // `new X; dup; ...; invokespecial`: the copy left below becomes the object.
                    match stack.last_mut() {
                        Some(top) if matches!(top.kind, Kind::New(_) | Kind::Builder(_)) => *top = made,
                        _ => out.push(Stmt::Line(format!("{};", made.code))),
                    }
                    return Ok(());
                }
                if let Some(e) = self.call(&m, recv, args, out)? {
                    stack.push(e);
                }
            }
            0xba => {
                let i = self.u16_at(pc);
                match self.spec.class.indy(i) {
                    Some(Indy::Concat { recipe, constants }) => {
                        let n = recipe.chars().filter(|&c| c == '\u{1}').count();
                        let mut args = Vec::new();
                        for _ in 0..n {
                            args.push(self.pop(stack)?);
                        }
                        args.reverse();
                        stack.push(concat(&recipe, &constants, args)?);
                    }
                    Some(Indy::Lambda(_)) => return Err("it makes a lambda".into()),
                    _ => return Err("an invokedynamic".into()),
                }
            }
            0xbb => {
                let c = self.spec.class.class_ref(self.u16_at(pc)).ok_or("a bad class reference")?;
                if c == "java/lang/StringBuilder" {
                    stack.push(Expr::new("", Kind::Builder(Vec::new())));
                } else {
                    stack.push(Expr::new("", Kind::New(c)));
                }
            }
            0xbd => {
                let n = self.pop(stack)?;
                let c = self.spec.class.class_ref(self.u16_at(pc)).unwrap_or_default();
                let Some(Lit::Int(n)) = n.lit else { return Err("an array of a size that isn't a constant".into()) };
                if c != "java/lang/String" || !(0..=64).contains(&n) {
                    return Err("an array other than a few strings".into());
                }
                stack.push(Expr::new("", Kind::Array(vec![None; n as usize])));
            }
            0xbe => {
                let a = self.pop(stack)?;
                if a.kind != Kind::Args {
                    return Err("the length of an array other than the arguments".into());
                }
                stack.push(Expr::op(format!("{}.len() as i32", a.recv()), Kind::I32));
            }
            0xc0 => {
                let c = self.spec.class.class_ref(self.u16_at(pc)).unwrap_or_default();
                let v = self.pop(stack)?;
                stack.push(checkcast(v, &c)?);
            }
            0xc1 => {
                let c = self.spec.class.class_ref(self.u16_at(pc)).unwrap_or_default();
                let v = self.pop(stack)?;
                if v.kind == Kind::Sender && mapping::simple_name(&c) == "Player" {
                    stack.push(Expr::new(format!("{}.player().is_some()", v.recv()), Kind::Bool));
                } else if v.kind == Kind::Sender && mapping::simple_name(&c) == "ConsoleCommandSender" {
                    stack.push(Expr::new(format!("{}.player().is_none()", v.recv()), Kind::Bool));
                } else {
                    return Err(format!("instanceof {}", mapping::simple_name(&c)));
                }
            }
            _ => return Err(format!("opcode {op:#04x}")),
        }
        Ok(())
    }

    fn get_field(&self, obj: &Expr, m: &MemberRef) -> R<Expr> {
        match obj.kind {
            Kind::Plugin if m.owner == self.spec.main => {
                let f = self.spec.fields.get(&m.name).ok_or("a field of the plugin with no Rust type")?;
                Ok(Expr::place(format!("plugin.{}", f.rust), f.kind.clone()))
            }
            // A listener's reference to the plugin.
            Kind::Listener if m.descriptor == format!("L{};", self.spec.main) => Ok(Expr::new("plugin", Kind::Plugin)),
            _ => Err(format!("reading the field {}.{}", mapping::simple_name(&m.owner), m.name)),
        }
    }

    /// A call: `Some` its value, or `None` when it became a statement.
    fn call(&mut self, m: &MemberRef, recv: Option<Expr>, args: Vec<Expr>, out: &mut Vec<Stmt>) -> R<Option<Expr>> {
        let ret = ret_letter(&m.descriptor);
        if let Some(e) = self.helper_call(m, recv.as_ref(), &args)? {
            if e.kind == Kind::Ignored {
                out.push(Stmt::Line(format!("{};", e.code)));
                return Ok(None);
            }
            return Ok(Some(e));
        }
        let e = match recv {
            Some(r) => self.method(m, r, &args)?,
            None => self.static_call(m, &args)?,
        };
        if ret == "V" || e.kind == Kind::Ignored {
            if !e.code.is_empty() {
                out.push(Stmt::Line(format!("{};", e.code)));
            }
            return Ok(if ret == "V" { None } else { Some(Expr::new("()", Kind::Ignored)) });
        }
        Ok(Some(e))
    }

    /// A call to one of the plugin's own methods, on the plugin, a listener or static: through its
    /// helper function. `None` when the call isn't the plugin's own.
    fn helper_call(&self, m: &MemberRef, recv: Option<&Expr>, args: &[Expr]) -> R<Option<Expr>> {
        let own_receiver = match recv {
            None => true,
            Some(r) => matches!(r.kind, Kind::Plugin | Kind::Listener),
        };
        let Some(helpers) = self.spec.helpers else { return Ok(None) };
        if !own_receiver || m.name.starts_with('<') || m.name.starts_with("lambda$") {
            return Ok(None);
        }
        // Only the plugin's own classes have helpers; the API's methods on the plugin (getConfig) don't.
        let helper = match helpers(m) {
            Ok(h) => h,
            Err(why) if why == "not the plugin's own" => return Ok(None),
            Err(why) => return Err(format!("it calls {}.{}, {why}", mapping::simple_name(&m.owner), m.name)),
        };
        let mut parts = vec!["plugin".to_owned(), "ctx".to_owned()];
        for (a, k) in args.iter().zip(&helper.params) {
            parts.push(match k {
                Kind::StrRef => self.s_ref(a)?,
                Kind::Bool => self.boolean(a)?,
                Kind::I32 | Kind::I64 | Kind::F64 | Kind::F32 => self.cast(a.clone(), k.clone()).code,
                Kind::Sender | Kind::Args if a.kind == *k => a.code.clone(),
                Kind::Player | Kind::Location | Kind::World | Kind::Block if a.kind == *k => format!("&{}", a.recv()),
                _ => return Err(format!("passing {:?} as {k:?} to {}", a.kind, m.name)),
            });
        }
        let code = format!("{}({})", helper.name, parts.join(", "));
        Ok(Some(match helper.ret {
            None => Expr::new(code, Kind::Ignored),
            Some(k) => Expr::new(code, k),
        }))
    }

    fn method(&mut self, m: &MemberRef, r: Expr, a: &[Expr]) -> R<Expr> {
        let n = m.name.as_str();
        let rc = r.recv();
        let none = || Err(format!("{}.{} has no Rust form here", mapping::simple_name(&m.owner), n));
        let stmt = |code: String| -> R<Expr> { Ok(Expr::new(code, Kind::Ignored)) };
        // Boxing and unboxing are the value itself.
        if matches!(n, "intValue" | "longValue" | "doubleValue" | "floatValue" | "booleanValue")
            && m.owner.starts_with("java/lang/")
        {
            return Ok(r);
        }
        Ok(match (&r.kind, n, a.len()) {
            // Players.
            (Kind::Player, "sendMessage", 1) => stmt(format!("{rc}.send_message({})", self.s_ref(&a[0])?))?,
            (Kind::Player, "sendActionBar", 1) => stmt(format!("{rc}.send_action_bar({})", self.s_ref(&a[0])?))?,
            (Kind::Player, "hasPermission", 1) => {
                Expr::new(format!("{rc}.has_permission({})", self.s_ref(&a[0])?), Kind::Bool)
            }
            (Kind::Player, "isOp", 0) => Expr::new(format!("{rc}.is_op()"), Kind::Bool),
            (Kind::Player, "isOnline", 0) => Expr::new(format!("{rc}.is_online()"), Kind::Bool),
            (Kind::Player, "isDead", 0) => Expr::new(format!("{rc}.is_dead()"), Kind::Bool),
            (Kind::Player, "getName" | "getDisplayName", 0) => Expr::place(format!("{rc}.name"), Kind::Str),
            (Kind::Player, "getUniqueId", 0) => Expr::new(format!("{rc}.uuid"), Kind::Uuid),
            (Kind::Uuid, "equals", 1) if a[0].kind == Kind::Uuid => {
                Expr::op(format!("{rc} == {}", a[0].recv()), Kind::Bool)
            }
            (Kind::Uuid, "toString", 0) => Expr::new(format!("{rc}.to_string()"), Kind::Str),
            (Kind::Player, "getLocation", 0) => Expr::place(format!("{rc}.location"), Kind::Location),
            (Kind::Player, "getWorld", 0) => Expr::new(format!("{rc}.world()"), Kind::World),
            (Kind::Player, "getGameMode", 0) => Expr::new(format!("{rc}.game_mode"), Kind::GameMode),
            (Kind::Player, "getHealth", 0) => Expr::op(format!("{rc}.health as f64"), Kind::F64),
            (Kind::Player, "teleport", 1) if a[0].kind == Kind::Location => {
                Expr::new(format!("{rc}.teleport(&{})", a[0].recv()), Kind::Bool)
            }
            (Kind::Player, "kickPlayer" | "kick", 1) => stmt(format!("{rc}.kick({})", self.s_ref(&a[0])?))?,
            (Kind::Player, "performCommand", 1) => {
                Expr::new(format!("{rc}.perform_command({})", self.s_ref(&a[0])?), Kind::Bool)
            }
            (Kind::Player, "setGameMode", 1) if a[0].kind == Kind::GameMode => {
                stmt(format!("{rc}.set_game_mode({})", a[0].code))?
            }
            (Kind::Player, "sendTitle", 5) => stmt(format!(
                "{rc}.send_title({}, {}, {}, {}, {})",
                self.s_ref(&a[0])?,
                self.s_ref(&a[1])?,
                a[2].code,
                a[3].code,
                a[4].code
            ))?,
            (Kind::Player, "sendTitle", 2) => {
                stmt(format!("{rc}.send_title({}, {}, 10, 70, 20)", self.s_ref(&a[0])?, self.s_ref(&a[1])?))?
            }
            // Lists and their for-each loops.
            (Kind::PlayerList | Kind::StrList, "iterator", 0) => {
                let item = if r.kind == Kind::PlayerList { Kind::Player } else { Kind::Str };
                let iter = if r.code.contains('(') { r.code.clone() } else { format!("{rc}.clone()") };
                Expr::new("", Kind::Iter(iter, Box::new(item)))
            }
            (Kind::Iter(iter, item), "hasNext", 0) => Expr::new("", Kind::HasNext(iter.clone(), item.clone())),
            (Kind::Iter(_, item), "next", 0) => Expr::new("", Kind::LoopItem(item.clone())),
            (Kind::PlayerList | Kind::StrList, "size", 0) => Expr::op(format!("{rc}.len() as i32"), Kind::I32),
            (Kind::PlayerList | Kind::StrList, "isEmpty", 0) => Expr::new(format!("{rc}.is_empty()"), Kind::Bool),
            (Kind::StrList, "contains", 1) if matches!(a[0].kind, Kind::Str | Kind::StrRef) => {
                Expr::new(format!("{rc}.iter().any(|s| s == {})", self.s_ref(&a[0])?), Kind::Bool)
            }
            (Kind::Str | Kind::StrRef, "hashCode", 0) => Expr::new("", Kind::Hash(Box::new(r.clone()))),
            (Kind::World, "getPlayers", 0) => Expr::new(format!("{rc}.players()"), Kind::PlayerList),
            // Command senders.
            (Kind::Sender, "sendMessage", 1) => stmt(format!("{rc}.send_message({})", self.s_ref(&a[0])?))?,
            (Kind::Sender, "hasPermission", 1) => {
                Expr::new(format!("{rc}.has_permission({})", self.s_ref(&a[0])?), Kind::Bool)
            }
            (Kind::Sender, "isOp", 0) => Expr::new(format!("{rc}.is_op()"), Kind::Bool),
            (Kind::Sender, "getName", 0) => Expr::new(format!("{rc}.name()"), Kind::StrRef),
            (Kind::Command, "getName", 0) => Expr::string(self.spec.command.ok_or("a command's name")?),
            // Events.
            (Kind::Event(ev), _, _) => self.event_method(ev, &rc, n, a)?,
            // Blocks and places.
            (Kind::Block, "getType", 0) => Expr::place(format!("{rc}.kind"), Kind::Str),
            (Kind::Block, "getLocation", 0) => Expr::new(format!("{rc}.pos.location()"), Kind::Location),
            (Kind::Block, "getWorld", 0) => Expr::new(format!("{rc}.pos.location().world()"), Kind::World),
            (Kind::Block, "getX", 0) => Expr::place(format!("{rc}.pos.x"), Kind::I32),
            (Kind::Block, "getY", 0) => Expr::place(format!("{rc}.pos.y"), Kind::I32),
            (Kind::Block, "getZ", 0) => Expr::place(format!("{rc}.pos.z"), Kind::I32),
            (Kind::BlockPos, "getX", 0) => Expr::place(format!("{rc}.x"), Kind::I32),
            (Kind::BlockPos, "getY", 0) => Expr::place(format!("{rc}.y"), Kind::I32),
            (Kind::BlockPos, "getZ", 0) => Expr::place(format!("{rc}.z"), Kind::I32),
            (Kind::BlockPos, "getLocation", 0) => Expr::new(format!("{rc}.location()"), Kind::Location),
            (Kind::BlockPos, "getWorld", 0) => Expr::new(format!("{rc}.location().world()"), Kind::World),
            (Kind::Location, "getX", 0) => Expr::place(format!("{rc}.x"), Kind::F64),
            (Kind::Location, "getY", 0) => Expr::place(format!("{rc}.y"), Kind::F64),
            (Kind::Location, "getZ", 0) => Expr::place(format!("{rc}.z"), Kind::F64),
            (Kind::Location, "getYaw", 0) => Expr::place(format!("{rc}.yaw"), Kind::F32),
            (Kind::Location, "getPitch", 0) => Expr::place(format!("{rc}.pitch"), Kind::F32),
            (Kind::Location, "getBlockX", 0) => Expr::new(format!("{rc}.block_x()"), Kind::I32),
            (Kind::Location, "getBlockY", 0) => Expr::new(format!("{rc}.block_y()"), Kind::I32),
            (Kind::Location, "getBlockZ", 0) => Expr::new(format!("{rc}.block_z()"), Kind::I32),
            (Kind::Location, "getWorld", 0) => Expr::new(format!("{rc}.world()"), Kind::World),
            (Kind::Location, "getBlock", 0) => Expr::new(format!("{rc}.block()"), Kind::BlockPos),
            (Kind::Location, "clone", 0) => Expr::new(format!("{rc}.clone()"), Kind::Location),
            (Kind::Location, "distance", 1) if a[0].kind == Kind::Location => {
                Expr::new(format!("{rc}.distance(&{})", a[0].recv()), Kind::F64)
            }
            (Kind::Location, "distanceSquared", 1) if a[0].kind == Kind::Location => {
                Expr::new(format!("{rc}.distance_squared(&{})", a[0].recv()), Kind::F64)
            }
            (Kind::Location, "add", 3) => {
                Expr::new(format!("{rc}.add({}, {}, {})", a[0].code, a[1].code, a[2].code), Kind::Location)
            }
            (Kind::World, "getName", 0) => Expr::place(format!("{rc}.name"), Kind::Str),
            (Kind::World, "strikeLightning" | "strikeLightningEffect", 1) if a[0].kind == Kind::Location => {
                stmt(format!("{rc}.strike_lightning(&{})", a[0].recv()))?
            }
            (Kind::World, "setTime" | "setFullTime", 1) => stmt(format!("{rc}.set_time({})", a[0].code))?,
            // The plugin.
            (Kind::Plugin, "getConfig", 0) => Expr::new("ctx.config()", Kind::Config),
            (Kind::Plugin, "getLogger", 0) => Expr::new("ctx.logger()", Kind::Logger),
            (Kind::Plugin, "getServer", 0) => Expr::new("server()", Kind::Server),
            (Kind::Plugin, "getDescription" | "getPluginMeta", 0) => Expr::new("ctx.description()", Kind::Description),
            (Kind::Plugin, "getName", 0) => Expr::place("ctx.description().name", Kind::Str),
            (Kind::Plugin, "saveConfig", 0) => stmt("ctx.save_config()".into())?,
            (Kind::Plugin, "reloadConfig", 0) => stmt("ctx.reload_config()".into())?,
            (Kind::Plugin, "saveDefaultConfig", 0) if self.spec.has_config => {
                stmt("ctx.save_default_config(include_str!(\"../config.yml\"))".into())?
            }
            (Kind::Description, "getVersion" | "version", 0) => Expr::place(format!("{rc}.version"), Kind::Str),
            (Kind::Description, "getName" | "name", 0) => Expr::place(format!("{rc}.name"), Kind::Str),
            (Kind::Description, "getDescription" | "description", 0) => {
                Expr::place(format!("{rc}.description"), Kind::Str)
            }
            (Kind::Description, "getWebsite" | "website", 0) => Expr::place(format!("{rc}.website"), Kind::Str),
            // The server.
            (Kind::Server, _, _) => self.server_call(n, a)?,
            // Config.
            (Kind::Config, _, _) => self.config_call(&rc, n, a)?,
            // Logging.
            (Kind::Logger, "info" | "warning" | "severe" | "fine", 1) => {
                stmt(format!("{rc}.{n}({})", self.s_ref(&a[0])?))?
            }
            // Text.
            (Kind::Str | Kind::StrRef, _, _) => self.string_call(&r, n, a)?,
            (Kind::Color, "toString", 0) => Expr::new(format!("{rc}.to_string()"), Kind::Str),
            (Kind::Builder(parts), "append", 1) => {
                let mut parts = parts.clone();
                parts.push(a[0].clone());
                Expr::new("", Kind::Builder(parts))
            }
            (Kind::Builder(parts), "toString", 0) => concat_parts(parts)?,
            _ => return none(),
        })
    }

    fn event_method(&self, ev: &'static str, rc: &str, n: &str, a: &[Expr]) -> R<Expr> {
        let field = |name: &str| -> R<Expr> {
            let kind = event_field(ev, name).ok_or_else(|| format!("{ev} has no {name}"))?;
            Ok(Expr::place(format!("{rc}.{name}"), kind))
        };
        let stmt = |code: String| -> R<Expr> { Ok(Expr::new(code, Kind::Ignored)) };
        match (n, a.len()) {
            ("getPlayer" | "getWhoClicked", 0) => field("player"),
            ("getEntity", 0) if event_field(ev, "entity").is_none() => field("player"),
            ("getEntity", 0) => Err("an entity".into()),
            ("getBlock" | "getClickedBlock" | "getBlockPlaced", 0) => field("block"),
            ("getBlockAgainst", 0) => field("against"),
            ("getFrom", 0) => field("from"),
            ("getTo", 0) => field("to"),
            ("getMessage", 0) => field("message"),
            ("getFormat", 0) => field("format"),
            ("getAction", 0) => field("action"),
            ("getSlot" | "getRawSlot", 0) => field("slot"),
            ("getPlugin", 0) => field("plugin"),
            ("isCancelled", 0) => Ok(Expr::new(format!("{rc}.is_cancelled()"), Kind::Bool)),
            ("setCancelled", 1) if is_cancellable(ev) => stmt(format!("{rc}.set_cancelled({})", self.boolean(&a[0])?)),
            ("setMessage", 1) if event_field(ev, "message").is_some() => {
                stmt(format!("{rc}.message = {}", self.s_owned(&a[0])?))
            }
            ("setFormat", 1) if event_field(ev, "format").is_some() => {
                stmt(format!("{rc}.format = {}", self.s_owned(&a[0])?))
            }
            ("setJoinMessage" | "setQuitMessage" | "joinMessage" | "quitMessage", 1) => {
                let (f, owner) = if n.contains("oin") {
                    ("join_message", "PlayerJoinEvent")
                } else {
                    ("quit_message", "PlayerQuitEvent")
                };
                if ev != owner {
                    return Err(format!("{ev} has no {f}"));
                }
                let v =
                    if a[0].kind == Kind::Null { "None".to_owned() } else { format!("Some({})", self.s_owned(&a[0])?) };
                stmt(format!("{rc}.{f} = {v}"))
            }
            _ => Err(format!("{ev}.{n} has no Rust form here")),
        }
    }

    fn server_call(&self, n: &str, a: &[Expr]) -> R<Expr> {
        let stmt = |code: String| -> R<Expr> { Ok(Expr::new(code, Kind::Ignored)) };
        match (n, a.len()) {
            ("broadcastMessage", 1) => stmt(format!("server().broadcast_message({})", self.s_ref(&a[0])?)),
            ("broadcast", 2) => stmt(format!("server().broadcast({}, {})", self.s_ref(&a[0])?, self.s_ref(&a[1])?)),
            ("dispatchCommand", 2) => {
                Ok(Expr::new(format!("server().dispatch_command({})", self.s_ref(&a[1])?), Kind::Bool))
            }
            ("getConsoleSender", 0) => Ok(Expr::new("server().console_sender()", Kind::Sender)),
            ("getOnlinePlayers", 0) => Ok(Expr::new("server().online_players()", Kind::PlayerList)),
            ("getServer", 0) => Ok(Expr::new("server()", Kind::Server)),
            ("getLogger", 0) => Ok(Expr::new("ctx.logger()", Kind::Logger)),
            ("getVersion", 0) => Ok(Expr::new("server().version()", Kind::Str)),
            ("getBukkitVersion" | "getMinecraftVersion", 0) => Ok(Expr::new("server().minecraft_version()", Kind::Str)),
            ("getName", 0) => Ok(Expr::new("server().name()", Kind::Str)),
            _ => Err(format!("Bukkit.{n} has no Rust form here")),
        }
    }

    fn config_call(&self, rc: &str, n: &str, a: &[Expr]) -> R<Expr> {
        let path = || self.s_ref(&a[0]);
        Ok(match (n, a.len()) {
            ("getString", 1) => Expr::new(format!("{rc}.string_or({}, \"\")", path()?), Kind::Str),
            ("getString", 2) => Expr::new(format!("{rc}.string_or({}, {})", path()?, self.s_ref(&a[1])?), Kind::Str),
            ("getInt", 1) => Expr::op(format!("{rc}.int_or({}, 0) as i32", path()?), Kind::I32),
            ("getInt", 2) => Expr::op(
                format!("{rc}.int_or({}, {}) as i32", path()?, self.cast(a[1].clone(), Kind::I64).code),
                Kind::I32,
            ),
            ("getLong", 1) => Expr::new(format!("{rc}.int_or({}, 0)", path()?), Kind::I64),
            ("getLong", 2) => Expr::new(format!("{rc}.int_or({}, {})", path()?, a[1].code), Kind::I64),
            ("getDouble", 1) => Expr::new(format!("{rc}.f64_or({}, 0.0)", path()?), Kind::F64),
            ("getDouble", 2) => Expr::new(format!("{rc}.f64_or({}, {})", path()?, a[1].code), Kind::F64),
            ("getBoolean", 1) => Expr::new(format!("{rc}.bool_or({}, false)", path()?), Kind::Bool),
            ("getBoolean", 2) => Expr::new(format!("{rc}.bool_or({}, {})", path()?, self.boolean(&a[1])?), Kind::Bool),
            ("getStringList", 1) => Expr::new(format!("{rc}.get_strings({})", path()?), Kind::StrList),
            ("contains" | "isSet", 1) => Expr::new(format!("{rc}.contains({})", path()?), Kind::Bool),
            ("set", 2) => {
                let v = match a[1].kind {
                    Kind::Str => self.s_owned(&a[1])?,
                    Kind::StrRef | Kind::I32 | Kind::I64 | Kind::F64 | Kind::Bool => a[1].code.clone(),
                    Kind::F32 => format!("{} as f64", a[1].recv()),
                    Kind::Null => return Err("removing a config key".into()),
                    ref k => return Err(format!("a config value of {k:?}")),
                };
                Expr::new(format!("{rc}.set({}, {v})", path()?), Kind::Ignored)
            }
            _ => return Err(format!("FileConfiguration.{n} has no Rust form here")),
        })
    }

    fn string_call(&self, r: &Expr, n: &str, a: &[Expr]) -> R<Expr> {
        let rc = r.recv();
        let text = |e: &Expr| matches!(e.kind, Kind::Str | Kind::StrRef);
        Ok(match (n, a.len()) {
            ("equals", 1) if text(&a[0]) => Expr::op(format!("{rc} == {}", a[0].recv()), Kind::Bool),
            ("equalsIgnoreCase", 1) => {
                Expr::new(format!("{rc}.eq_ignore_ascii_case({})", self.s_ref(&a[0])?), Kind::Bool)
            }
            ("replace", 2) if text(&a[0]) && text(&a[1]) => {
                Expr::new(format!("{rc}.replace({}, {})", self.s_ref(&a[0])?, self.s_ref(&a[1])?), Kind::Str)
            }
            ("toLowerCase", 0) => Expr::new(format!("{rc}.to_lowercase()"), Kind::Str),
            ("toUpperCase", 0) => Expr::new(format!("{rc}.to_uppercase()"), Kind::Str),
            ("trim" | "strip", 0) => Expr::new(format!("{rc}.trim().to_owned()"), Kind::Str),
            ("isEmpty", 0) => Expr::new(format!("{rc}.is_empty()"), Kind::Bool),
            ("isBlank", 0) => Expr::new(format!("{rc}.trim().is_empty()"), Kind::Bool),
            ("length", 0) => Expr::op(format!("{rc}.chars().count() as i32"), Kind::I32),
            ("startsWith", 1) => Expr::new(format!("{rc}.starts_with({})", self.s_ref(&a[0])?), Kind::Bool),
            ("endsWith", 1) => Expr::new(format!("{rc}.ends_with({})", self.s_ref(&a[0])?), Kind::Bool),
            ("contains", 1) => Expr::new(format!("{rc}.contains({})", self.s_ref(&a[0])?), Kind::Bool),
            ("toString", 0) => r.clone(),
            // Adventure components are plain text here.
            ("append", 1) if text(&a[0]) || a[0].kind == Kind::Color => {
                Expr::new(format!("format!(\"{{}}{{}}\", {}, {})", r.code, a[0].code), Kind::Str)
            }
            ("color", 1) if a[0].kind == Kind::Color => {
                Expr::new(format!("format!(\"{{}}{{}}\", {}, {})", a[0].code, r.code), Kind::Str)
            }
            _ => return Err(format!("String.{n} has no Rust form here")),
        })
    }

    fn static_call(&self, m: &MemberRef, a: &[Expr]) -> R<Expr> {
        let owner = mapping::simple_name(&m.owner);
        let n = m.name.as_str();
        // Boxing is the value itself.
        if n == "valueOf" && m.owner.starts_with("java/lang/") && owner != "String" && a.len() == 1 {
            return Ok(a[0].clone());
        }
        Ok(match (owner, n, a.len()) {
            ("Bukkit", _, _) => self.server_call(n, a)?,
            ("Component", "text", 1) if matches!(a[0].kind, Kind::Str | Kind::StrRef) => a[0].clone(),
            ("Component", "text", 2) if matches!(a[0].kind, Kind::Str | Kind::StrRef) && a[1].kind == Kind::Color => {
                Expr::new(format!("format!(\"{{}}{{}}\", {}, {})", a[1].code, a[0].code), Kind::Str)
            }
            ("Component", "empty", 0) => Expr::string(""),
            ("ChatColor", "translateAlternateColorCodes", 2) if a[0].kind == Kind::Char => {
                Expr::new(format!("color::translate({}, {})", a[0].code, self.s_ref(&a[1])?), Kind::Str)
            }
            ("ChatColor", "stripColor", 1) => Expr::new(format!("color::strip({})", self.s_ref(&a[0])?), Kind::Str),
            ("String", "valueOf", 1) => Expr::new(format!("{}.to_string()", a[0].recv()), Kind::Str),
            ("String", "join", 2) if matches!(a[1].kind, Kind::Args | Kind::StrList) => {
                Expr::new(format!("{}.join({})", a[1].recv(), self.s_ref(&a[0])?), Kind::Str)
            }
            ("Math", "max" | "min", 2) => {
                let (x, y) = self.same_number(a[0].clone(), a[1].clone())?;
                let k = x.kind.clone();
                Expr::new(format!("{}.{n}({})", x.recv(), y.code), k)
            }
            ("Math", "abs", 1) => Expr::new(format!("{}.abs()", a[0].recv()), a[0].kind.clone()),
            ("System", "currentTimeMillis", 0) => Expr::new(
                "std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)",
                Kind::I64,
            ),
            ("System", "nanoTime", 0) => Expr::new(
                "std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as i64)",
                Kind::I64,
            ),
            ("Arrays", "asList", 1) | ("List", "of", 1) if matches!(a[0].kind, Kind::Array(_)) => {
                let Kind::Array(items) = &a[0].kind else { unreachable!() };
                let mut out = Vec::new();
                for i in items {
                    out.push(self.s_owned(i.as_ref().ok_or("an array with a gap")?)?);
                }
                Expr::new(format!("vec![{}]", out.join(", ")), Kind::StrList)
            }
            ("List" | "Collections", "of" | "singletonList", _)
                if a.iter().all(|x| matches!(x.kind, Kind::Str | Kind::StrRef)) =>
            {
                let items = a.iter().map(|i| self.s_owned(i)).collect::<R<Vec<_>>>()?;
                Expr::new(format!("vec![{}]", items.join(", ")), Kind::StrList)
            }
            ("Collections", "emptyList", 0) => Expr::new("Vec::new()", Kind::StrList),
            ("Objects", "equals", 2)
                if matches!(a[0].kind, Kind::Str | Kind::StrRef) && matches!(a[1].kind, Kind::Str | Kind::StrRef) =>
            {
                Expr::op(format!("{} == {}", a[0].recv(), a[1].recv()), Kind::Bool)
            }
            _ => return Err(format!("{owner}.{n} has no Rust form here")),
        })
    }
}

/// The kind of a Horizon event's field, from horizon-plugin-api's `event.rs`.
pub fn event_field(event: &str, field: &str) -> Option<Kind> {
    use Kind::*;
    let fields: &[(&str, Kind)] = match event {
        "PlayerJoinEvent" => &[("player", Player)],
        "PlayerQuitEvent" => &[("player", Player)],
        "PlayerMoveEvent" => &[("player", Player), ("from", Location), ("to", Location)],
        "PlayerChangedWorldEvent" => &[("player", Player), ("from", Str)],
        "PlayerDeathEvent" | "PlayerRespawnEvent" => &[("player", Player)],
        "PlayerGameModeChangeEvent" => &[("player", Player), ("previous", GameMode)],
        "AsyncPlayerChatEvent" => &[("player", Player), ("message", Str), ("format", Str)],
        "PlayerInteractEvent" => &[("player", Player), ("action", Str)],
        "BlockBreakEvent" => &[("player", Player), ("block", Block)],
        "BlockPlaceEvent" => &[("player", Player), ("block", BlockPos), ("against", BlockPos)],
        "PlayerBucketFillEvent" | "PlayerBucketEmptyEvent" => &[("player", Player), ("block", BlockPos)],
        "PrePlayerAttackEntityEvent" => &[("player", Player)],
        "InventoryClickEvent" => &[("player", Player), ("window", I32), ("slot", I32), ("button", I32), ("mode", I32)],
        "InventoryCloseEvent" => &[("player", Player), ("window", I32)],
        "BlockFromToEvent" => &[("block", BlockPos), ("to", BlockPos)],
        "BlockSpreadEvent" => &[("source", BlockPos), ("block", BlockPos)],
        "PluginEnableEvent" | "PluginDisableEvent" => &[("plugin", Str)],
        "EntityExplodeEvent" => &[("location", Location)],
        _ => &[],
    };
    fields.iter().find(|(n, _)| *n == field).map(|(_, k)| k.clone())
}

fn is_cancellable(event: &str) -> bool {
    matches!(
        event,
        "PlayerMoveEvent"
            | "AsyncPlayerChatEvent"
            | "PlayerInteractEvent"
            | "BlockBreakEvent"
            | "BlockPlaceEvent"
            | "PlayerBucketFillEvent"
            | "PlayerBucketEmptyEvent"
            | "PrePlayerAttackEntityEvent"
            | "EntitySpawnEvent"
            | "EntityExplodeEvent"
            | "BlockFromToEvent"
            | "BlockSpreadEvent"
            | "BlockPistonEvent"
    )
}

/// A static field: the API's constants.
fn static_field(m: &MemberRef) -> R<Expr> {
    let lower = m.name.to_ascii_lowercase();
    Ok(match mapping::simple_name(&m.owner) {
        "Material" => Expr::string(&format!("minecraft:{lower}")),
        "ChatColor" | "NamedTextColor" => Expr::new(format!("ChatColor::{}", mapping::camel(&lower)), Kind::Color),
        "GameMode" => Expr::new(format!("GameMode::{}", mapping::camel(&lower)), Kind::GameMode),
        "Action" if m.owner.starts_with("org/bukkit/event/block/") => {
            if lower == "left_click_air" || lower == "physical" {
                return Err(format!("Action.{} (no Horizon action)", m.name));
            }
            Expr::string(&lower)
        }
        _ => return Err(format!("the static field {}.{}", mapping::simple_name(&m.owner), m.name)),
    })
}

fn checkcast(v: Expr, class: &str) -> R<Expr> {
    let simple = mapping::simple_name(class);
    match (&v.kind, simple) {
        (Kind::Sender, "Player") => Ok(Expr::new(format!("{}.player().expect(\"a player\")", v.recv()), Kind::Player)),
        (Kind::Str | Kind::StrRef, "String" | "CharSequence") => Ok(v),
        (Kind::Player, "Player") | (Kind::Sender, "CommandSender") => Ok(v),
        (Kind::LoopItem(k), "Player") if **k == Kind::Player => Ok(v),
        (Kind::LoopItem(k), "String") if **k == Kind::Str => Ok(v),
        (Kind::Str | Kind::StrRef, "Component" | "TextComponent") => Ok(v),
        (
            Kind::I32 | Kind::I64 | Kind::F64 | Kind::F32 | Kind::Bool,
            "Integer" | "Long" | "Double" | "Float" | "Boolean" | "Number",
        ) => Ok(v),
        _ => Err(format!("a cast of {:?} to {simple}", v.kind)),
    }
}

fn construct(m: &MemberRef, recv: Expr, a: Vec<Expr>, tr: &Tr) -> R<Expr> {
    let Kind::New(c) = &recv.kind else {
        return match recv.kind {
            Kind::Builder(_) if a.is_empty() => Ok(Expr::new("", Kind::Builder(Vec::new()))),
            Kind::Builder(_) if a.len() == 1 => Ok(Expr::new("", Kind::Builder(a))),
            // super() in a constructor.
            _ => Err("a constructor call".into()),
        };
    };
    let _ = m;
    match (mapping::simple_name(c), a.len()) {
        ("Location", 4) if a[0].kind == Kind::World => Ok(Expr::new(
            format!("Location::new(&{}.name, {}, {}, {})", a[0].recv(), a[1].code, a[2].code, a[3].code),
            Kind::Location,
        )),
        ("Location", 6) if a[0].kind == Kind::World => Ok(Expr::new(
            format!(
                "Location::new(&{}.name, {}, {}, {}).with_rotation({}, {})",
                a[0].recv(),
                a[1].code,
                a[2].code,
                a[3].code,
                a[4].code,
                a[5].code
            ),
            Kind::Location,
        )),
        ("String", 1) if matches!(a[0].kind, Kind::Str | Kind::StrRef) => Ok(Expr::new(tr.s_owned(&a[0])?, Kind::Str)),
        (s, _) => Err(format!("new {s}")),
    }
}

/// A Java 9+ string concatenation: `recipe` has `\u{1}` for each value and `\u{2}` for each constant.
fn concat(recipe: &str, constants: &[String], args: Vec<Expr>) -> R<Expr> {
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut args = args.into_iter();
    let mut constants = constants.iter();
    for c in recipe.chars() {
        match c {
            '\u{1}' => {
                if !lit.is_empty() {
                    parts.push(Expr::string(&std::mem::take(&mut lit)));
                }
                parts.push(args.next().ok_or("a concatenation missing a value")?);
            }
            '\u{2}' => lit.push_str(constants.next().ok_or("a concatenation missing a constant")?),
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        parts.push(Expr::string(&lit));
    }
    concat_parts(&parts)
}

/// `format!` from text and values.
fn concat_parts(parts: &[Expr]) -> R<Expr> {
    let mut fmt = String::new();
    let mut args = Vec::new();
    for p in parts {
        match (&p.kind, &p.lit) {
            (Kind::StrRef, Some(Lit::Str(s))) => fmt.push_str(&s.replace('{', "{{").replace('}', "}}")),
            (
                Kind::Str
                | Kind::StrRef
                | Kind::I32
                | Kind::I64
                | Kind::F32
                | Kind::F64
                | Kind::Bool
                | Kind::Char
                | Kind::Color
                | Kind::Uuid,
                _,
            ) => {
                fmt.push_str("{}");
                // format! shows a value as to_string() would.
                let code = p.code.strip_suffix(".to_string()").filter(|c| !c.is_empty()).unwrap_or(&p.code);
                args.push(code.to_owned());
            }
            (k, _) => return Err(format!("{k:?} in a string")),
        }
    }
    if args.is_empty() {
        return Ok(Expr::string(&fmt.replace("{{", "{").replace("}}", "}")));
    }
    let fmt = format!("{fmt:?}");
    Ok(Expr::new(format!("format!({fmt}, {})", args.join(", ")), Kind::Str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_snake_case() {
        assert_eq!(snake("joinCount"), "join_count");
        assert_eq!(snake("type"), "type_");
        assert_eq!(snake("URL"), "u_r_l");
    }

    #[test]
    fn identifiers_are_found_outside_strings() {
        assert!(uses_ident("e.player.send_message(\"x\");", "e"));
        assert!(!uses_ident("server().broadcast_message(\"e\");", "e"));
        assert!(!uses_ident("let event = 1;", "e"));
        assert!(uses_ident("plugin.joins += 1;", "plugin"));
    }

    #[test]
    fn concatenation_becomes_format() {
        let e = concat(
            "\u{1}Joins so far: \u{1}",
            &[],
            vec![Expr::new("ChatColor::Yellow", Kind::Color), Expr::place("plugin.joins", Kind::I32)],
        )
        .unwrap();
        assert_eq!(e.code, "format!(\"{}Joins so far: {}\", ChatColor::Yellow, plugin.joins)");
        let e = concat("only {text}", &[], vec![]).unwrap();
        assert_eq!(e.code, "\"only {text}\"");
    }
}
