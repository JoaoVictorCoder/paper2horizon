//! Java `.class` files: as much as the converter reads. The class, its super class and interfaces,
//! its methods with their annotations, and what each method's code refers to: the methods it calls,
//! the fields it reads, the classes it makes and the strings it holds.

use std::collections::BTreeSet;

#[derive(Clone, Debug, Default)]
pub struct Class {
    /// Internal name: `com/example/Greeter`.
    pub name: String,
    pub super_name: Option<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pool: Pool,
    bootstraps: Vec<(u16, Vec<u16>)>,
}

#[derive(Clone, Debug, Default)]
pub struct Field {
    pub name: String,
    /// `I`, `Ljava/lang/String;`.
    pub descriptor: String,
    pub is_static: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Method {
    pub name: String,
    /// `(Lorg/bukkit/event/player/PlayerJoinEvent;)V`.
    pub descriptor: String,
    pub is_static: bool,
    pub annotations: Vec<Annotation>,
    pub code: Code,
    /// The bytecode, for the translator; `None` for abstract and native methods.
    pub body: Option<Body>,
}

#[derive(Clone, Debug, Default)]
pub struct Body {
    pub code: Vec<u8>,
    /// How many exception handlers (`try`/`catch`, `synchronized`) it has.
    pub handlers: usize,
    /// The local variables' names, where the class was compiled with them (`-g`, Gradle's and Maven's
    /// default).
    pub locals: Vec<LocalVar>,
}

#[derive(Clone, Debug, Default)]
pub struct LocalVar {
    pub slot: u16,
    pub start: u16,
    pub len: u16,
    pub name: String,
    pub descriptor: String,
}

/// A constant `ldc` loads.
#[derive(Clone, Debug, PartialEq)]
pub enum Const {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Str(String),
    /// A class literal.
    Class(String),
}

/// What an `invokedynamic` makes.
#[derive(Clone, Debug, PartialEq)]
pub enum Indy {
    /// String concatenation: the recipe (`\u{1}` for each argument, `\u{2}` for each constant) and its
    /// constants.
    Concat {
        recipe: String,
        constants: Vec<String>,
    },
    /// A lambda or method reference, and the method it runs.
    Lambda(MemberRef),
    Other,
}

impl Class {
    pub fn constant(&self, i: u16) -> Option<Const> {
        Some(match self.pool.get(i)? {
            Constant::Int(v) => Const::Int(*v),
            Constant::Long(v) => Const::Long(*v),
            Constant::Float(v) => Const::Float(*v),
            Constant::Double(v) => Const::Double(*v),
            Constant::String(s) => Const::Str(self.pool.utf8(*s)?.to_owned()),
            Constant::Class(_) => Const::Class(self.pool.class(i)?.to_owned()),
            _ => return None,
        })
    }

    pub fn member(&self, i: u16) -> Option<MemberRef> {
        self.pool.member(i)
    }

    pub fn class_ref(&self, i: u16) -> Option<String> {
        self.pool.class(i).map(str::to_owned)
    }

    pub fn indy(&self, i: u16) -> Option<Indy> {
        let Constant::InvokeDynamic { bootstrap } = self.pool.get(i)? else { return None };
        let (handle, args) = self.bootstraps.get(*bootstrap as usize)?;
        let factory = match self.pool.get(*handle)? {
            Constant::MethodHandle { reference } => self.pool.member(*reference)?,
            _ => return None,
        };
        if factory.owner == "java/lang/invoke/StringConcatFactory" {
            let mut texts = args.iter().filter_map(|a| match self.pool.get(*a) {
                Some(Constant::String(s)) => self.pool.utf8(*s).map(str::to_owned),
                _ => None,
            });
            let recipe = texts.next()?;
            return Some(Indy::Concat { recipe, constants: texts.collect() });
        }
        for a in args {
            if let Some(Constant::MethodHandle { reference }) = self.pool.get(*a) {
                if let Some(target) = self.pool.member(*reference) {
                    return Some(Indy::Lambda(target));
                }
            }
        }
        Some(Indy::Other)
    }

    pub fn method(&self, name: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.name == name)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotation {
    /// `Lorg/bukkit/event/EventHandler;`.
    pub kind: String,
    /// Each element as text: an enum's constant name, a number, a string or a boolean
    /// (`true`/`false`).
    pub values: Vec<(String, String)>,
}

/// What a method's code refers to, in the order it first does.
#[derive(Clone, Debug, Default)]
pub struct Code {
    pub calls: Vec<MemberRef>,
    pub fields: Vec<MemberRef>,
    pub news: Vec<String>,
    pub strings: Vec<String>,
    /// Methods of this class its lambdas and method references run (`lambda$onEnable$0`).
    pub lambdas: Vec<String>,
    /// The code's steps that matter to find which command a `setExecutor` is for: in order.
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberRef {
    pub owner: String,
    pub name: String,
    pub descriptor: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    String(String),
    New(String),
    Call(MemberRef),
    /// `this` loaded (`aload_0` in an instance method).
    This,
    /// A class literal (`BlockBreakEvent.class`).
    ClassLiteral(String),
    /// A `long` constant (a scheduler's ticks).
    Long(i64),
    /// A field read (`getfield`, `getstatic`).
    Field(MemberRef),
    /// An object stored to a local variable, or loaded from one (other than `this`).
    Store(u16),
    Load(u16),
    /// A lambda or method reference made, and the method it runs.
    Lambda(MemberRef),
}

#[derive(Clone, Debug)]
enum Constant {
    Utf8(String),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Class(u16),
    String(u16),
    Member {
        class: u16,
        name_type: u16,
    },
    NameType {
        name: u16,
        descriptor: u16,
    },
    MethodHandle {
        reference: u16,
    },
    InvokeDynamic {
        bootstrap: u16,
    },
    Other,
    /// The second slot a long or double takes.
    Unusable,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or("the class file ends early")?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[derive(Clone, Debug, Default)]
struct Pool(Vec<Constant>);

impl Pool {
    fn get(&self, i: u16) -> Option<&Constant> {
        self.0.get(i as usize)
    }
    fn utf8(&self, i: u16) -> Option<&str> {
        match self.get(i)? {
            Constant::Utf8(s) => Some(s),
            _ => None,
        }
    }
    fn class(&self, i: u16) -> Option<&str> {
        match self.get(i)? {
            Constant::Class(n) => self.utf8(*n),
            _ => None,
        }
    }
    fn name_type(&self, i: u16) -> Option<(&str, &str)> {
        match self.get(i)? {
            Constant::NameType { name, descriptor } => Some((self.utf8(*name)?, self.utf8(*descriptor)?)),
            _ => None,
        }
    }
    fn member(&self, i: u16) -> Option<MemberRef> {
        match self.get(i)? {
            Constant::Member { class, name_type } => {
                let (name, descriptor) = self.name_type(*name_type)?;
                Some(MemberRef {
                    owner: self.class(*class)?.to_owned(),
                    name: name.to_owned(),
                    descriptor: descriptor.to_owned(),
                })
            }
            _ => None,
        }
    }
}

/// Java's "modified UTF-8": standard UTF-8 but for `\0` as two bytes and characters outside the
/// basic plane as two surrogates. Read leniently: what isn't valid becomes U+FFFD.
fn modified_utf8(bytes: &[u8]) -> String {
    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            units.push(b as u16);
            i += 1;
        } else if b & 0xe0 == 0xc0 && i + 1 < bytes.len() {
            units.push(((b as u16 & 0x1f) << 6) | (bytes[i + 1] as u16 & 0x3f));
            i += 2;
        } else if b & 0xf0 == 0xe0 && i + 2 < bytes.len() {
            units.push(((b as u16 & 0x0f) << 12) | ((bytes[i + 1] as u16 & 0x3f) << 6) | (bytes[i + 2] as u16 & 0x3f));
            i += 3;
        } else {
            units.push(0xfffd);
            i += 1;
        }
    }
    String::from_utf16_lossy(&units)
}

pub fn parse(bytes: &[u8]) -> Result<Class, String> {
    let mut r = Reader { bytes, at: 0 };
    if r.u32()? != 0xcafe_babe {
        return Err("not a class file".into());
    }
    r.take(4)?; // version
    let count = r.u16()?;
    let mut pool = vec![Constant::Unusable];
    while pool.len() < count as usize {
        let tag = r.u8()?;
        let c = match tag {
            1 => {
                let n = r.u16()? as usize;
                Constant::Utf8(modified_utf8(r.take(n)?))
            }
            3 => Constant::Int(r.u32()? as i32),
            4 => Constant::Float(f32::from_bits(r.u32()?)),
            5 | 6 => {
                let bits = ((r.u32()? as u64) << 32) | r.u32()? as u64;
                pool.push(if tag == 5 { Constant::Long(bits as i64) } else { Constant::Double(f64::from_bits(bits)) });
                // A long or double takes two slots.
                Constant::Unusable
            }
            7 => Constant::Class(r.u16()?),
            8 => Constant::String(r.u16()?),
            9..=11 => Constant::Member { class: r.u16()?, name_type: r.u16()? },
            12 => Constant::NameType { name: r.u16()?, descriptor: r.u16()? },
            15 => {
                r.u8()?;
                Constant::MethodHandle { reference: r.u16()? }
            }
            16 | 19 | 20 => {
                r.u16()?;
                Constant::Other
            }
            17 | 18 => {
                let bootstrap = r.u16()?;
                r.u16()?;
                Constant::InvokeDynamic { bootstrap }
            }
            t => return Err(format!("unknown constant tag {t}")),
        };
        pool.push(c);
    }
    let pool = Pool(pool);
    r.u16()?; // access
    let name = pool.class(r.u16()?).ok_or("no class name")?.to_owned();
    let super_name = pool.class(r.u16()?).map(str::to_owned);
    let mut interfaces = Vec::new();
    for _ in 0..r.u16()? {
        interfaces.extend(pool.class(r.u16()?).map(str::to_owned));
    }
    let mut fields = Vec::new();
    for _ in 0..r.u16()? {
        let access = r.u16()?;
        let name = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        let descriptor = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        skip_attributes(&mut r)?;
        fields.push(Field { name, descriptor, is_static: access & 0x0008 != 0 });
    }
    let mut raw_methods = Vec::new();
    for _ in 0..r.u16()? {
        let access = r.u16()?;
        let name = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        let descriptor = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        let mut annotations = Vec::new();
        let mut code = None;
        for _ in 0..r.u16()? {
            let attr = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
            let len = r.u32()? as usize;
            let body = r.take(len)?;
            match attr.as_str() {
                "RuntimeVisibleAnnotations" | "RuntimeInvisibleAnnotations" => {
                    annotations.extend(read_annotations(body, &pool).unwrap_or_default());
                }
                "Code" => code = Some(body),
                _ => {}
            }
        }
        raw_methods.push((name, descriptor, access & 0x0008 != 0, annotations, code));
    }
    // The class's attributes, for BootstrapMethods (lambdas and string concatenation).
    let mut bootstraps: Vec<(u16, Vec<u16>)> = Vec::new();
    for _ in 0..r.u16()? {
        let attr = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        let len = r.u32()? as usize;
        let body = r.take(len)?;
        if attr == "BootstrapMethods" {
            let mut b = Reader { bytes: body, at: 0 };
            for _ in 0..b.u16()? {
                let handle = b.u16()?;
                let args = (0..b.u16()?).map(|_| b.u16()).collect::<Result<_, _>>()?;
                bootstraps.push((handle, args));
            }
        }
    }
    let methods = raw_methods
        .into_iter()
        .map(|(name, descriptor, is_static, annotations, code)| {
            let body = code.map(|c| read_body(c, &pool)).transpose()?;
            let code = code.map(|c| read_code(c, &pool, &bootstraps)).transpose()?.unwrap_or_default();
            Ok(Method { name, descriptor, is_static, annotations, code, body })
        })
        .collect::<Result<_, String>>()?;
    Ok(Class { name, super_name, interfaces, fields, methods, pool, bootstraps })
}

/// A `Code` attribute's bytecode, exception handlers and local variable names.
fn read_body(body: &[u8], pool: &Pool) -> Result<Body, String> {
    let mut r = Reader { bytes: body, at: 0 };
    r.take(4)?; // max stack and locals
    let len = r.u32()? as usize;
    let code = r.take(len)?.to_vec();
    let handlers = r.u16()? as usize;
    r.take(handlers * 8)?;
    let mut locals = Vec::new();
    for _ in 0..r.u16()? {
        let attr = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        let n = r.u32()? as usize;
        let data = r.take(n)?;
        if attr == "LocalVariableTable" {
            let mut l = Reader { bytes: data, at: 0 };
            for _ in 0..l.u16()? {
                let (start, len) = (l.u16()?, l.u16()?);
                let name = pool.utf8(l.u16()?).unwrap_or_default().to_owned();
                let descriptor = pool.utf8(l.u16()?).unwrap_or_default().to_owned();
                locals.push(LocalVar { slot: l.u16()?, start, len, name, descriptor });
            }
        }
    }
    Ok(Body { code, handlers, locals })
}

fn skip_attributes(r: &mut Reader) -> Result<(), String> {
    for _ in 0..r.u16()? {
        r.u16()?;
        let n = r.u32()? as usize;
        r.take(n)?;
    }
    Ok(())
}

fn read_annotations(body: &[u8], pool: &Pool) -> Result<Vec<Annotation>, String> {
    let mut r = Reader { bytes: body, at: 0 };
    (0..r.u16()?).map(|_| read_annotation(&mut r, pool)).collect()
}

fn read_annotation(r: &mut Reader, pool: &Pool) -> Result<Annotation, String> {
    let kind = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
    let mut values = Vec::new();
    for _ in 0..r.u16()? {
        let name = pool.utf8(r.u16()?).unwrap_or_default().to_owned();
        values.push((name, read_element(r, pool)?));
    }
    Ok(Annotation { kind, values })
}

fn read_element(r: &mut Reader, pool: &Pool) -> Result<String, String> {
    let tag = r.u8()?;
    Ok(match tag {
        b'Z' => match pool.get(r.u16()?) {
            Some(Constant::Int(0)) => "false".into(),
            _ => "true".into(),
        },
        b'B' | b'C' | b'I' | b'S' => match pool.get(r.u16()?) {
            Some(Constant::Int(v)) => v.to_string(),
            _ => String::new(),
        },
        b'D' | b'F' | b'J' => {
            r.u16()?;
            String::new()
        }
        b's' => pool.utf8(r.u16()?).unwrap_or_default().to_owned(),
        b'e' => {
            r.u16()?;
            pool.utf8(r.u16()?).unwrap_or_default().to_owned()
        }
        b'c' => pool.utf8(r.u16()?).unwrap_or_default().to_owned(),
        b'@' => {
            read_annotation(r, pool)?;
            String::new()
        }
        b'[' => {
            let items = (0..r.u16()?).map(|_| read_element(r, pool)).collect::<Result<Vec<_>, _>>()?;
            items.join(",")
        }
        t => return Err(format!("unknown annotation element {}", t as char)),
    })
}

/// The length of the instruction at `pc` (its opcode included); `None` for an unknown opcode.
pub fn instruction_len(code: &[u8], pc: usize) -> Option<usize> {
    let op = *code.get(pc)?;
    let fixed = match op {
        0x00..=0x0f | 0x1a..=0x35 | 0x3b..=0x83 | 0x85..=0x98 | 0xac..=0xb1 | 0xbe | 0xbf | 0xc2 | 0xc3 => 1,
        0x10 | 0x12 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 2,
        0x11 | 0x13 | 0x14 | 0x84 | 0x99..=0xa8 | 0xb2..=0xb8 | 0xbb | 0xbd | 0xc0 | 0xc1 | 0xc6 | 0xc7 => 3,
        0xc5 => 4,
        0xb9 | 0xba | 0xc8 | 0xc9 => 5,
        0xc4 => return Some(if *code.get(pc + 1)? == 0x84 { 6 } else { 4 }),
        0xaa => {
            let base = (pc + 4) & !3;
            let lo = i32::from_be_bytes(code.get(base + 4..base + 8)?.try_into().ok()?);
            let hi = i32::from_be_bytes(code.get(base + 8..base + 12)?.try_into().ok()?);
            let n = (hi as i64 - lo as i64 + 1).max(0) as usize;
            return Some(base + 12 + 4 * n - pc);
        }
        0xab => {
            let base = (pc + 4) & !3;
            let n = i32::from_be_bytes(code.get(base + 4..base + 8)?.try_into().ok()?).max(0) as usize;
            return Some(base + 8 + 8 * n - pc);
        }
        _ => return None,
    };
    Some(fixed)
}

fn read_code(body: &[u8], pool: &Pool, bootstraps: &[(u16, Vec<u16>)]) -> Result<Code, String> {
    let mut r = Reader { bytes: body, at: 0 };
    r.take(4)?; // max stack and locals
    let len = r.u32()? as usize;
    let code = r.take(len)?;
    let mut out = Code::default();
    let mut seen_calls = BTreeSet::new();
    let mut seen_fields = BTreeSet::new();
    let mut pc = 0;
    let u16_at = |pc: usize| u16::from_be_bytes([code[pc + 1], code[pc + 2]]);
    while pc < code.len() {
        let op = code[pc];
        let Some(n) = instruction_len(code, pc) else { break };
        if pc + n > code.len() {
            break;
        }
        match op {
            0x2a => out.steps.push(Step::This),
            0x2b..=0x2d => out.steps.push(Step::Load((op - 0x2a) as u16)),
            0x19 => out.steps.push(Step::Load(code[pc + 1] as u16)),
            0x4b..=0x4e => out.steps.push(Step::Store((op - 0x4b) as u16)),
            0x3a => out.steps.push(Step::Store(code[pc + 1] as u16)),
            0x09 | 0x0a => out.steps.push(Step::Long(op as i64 - 9)),
            0x14 => {
                if let Some(Constant::Long(v)) = pool.get(u16_at(pc)) {
                    out.steps.push(Step::Long(*v));
                }
            }
            0x12 | 0x13 => {
                let i = if op == 0x12 { code[pc + 1] as u16 } else { u16_at(pc) };
                match pool.get(i) {
                    Some(Constant::String(s)) => {
                        let s = pool.utf8(*s).unwrap_or_default().to_owned();
                        out.steps.push(Step::String(s.clone()));
                        if !out.strings.contains(&s) {
                            out.strings.push(s);
                        }
                    }
                    Some(Constant::Class(_)) => {
                        out.steps.push(Step::ClassLiteral(pool.class(i).unwrap_or_default().to_owned()))
                    }
                    _ => {}
                }
            }
            0xb2..=0xb5 => {
                if let Some(m) = pool.member(u16_at(pc)) {
                    if op == 0xb2 || op == 0xb4 {
                        out.steps.push(Step::Field(m.clone()));
                    }
                    if seen_fields.insert(m.clone()) {
                        out.fields.push(m);
                    }
                }
            }
            0xb6..=0xb9 => {
                if let Some(m) = pool.member(u16_at(pc)) {
                    out.steps.push(Step::Call(m.clone()));
                    if seen_calls.insert(m.clone()) {
                        out.calls.push(m);
                    }
                }
            }
            0xba => {
                if let Some(Constant::InvokeDynamic { bootstrap, .. }) = pool.get(u16_at(pc)) {
                    read_indy(pool, bootstraps.get(*bootstrap as usize), &mut out);
                }
            }
            0xbb => {
                if let Some(c) = pool.class(u16_at(pc)) {
                    out.steps.push(Step::New(c.to_owned()));
                    if !out.news.iter().any(|n| n == c) {
                        out.news.push(c.to_owned());
                    }
                }
            }
            _ => {}
        }
        pc += n;
    }
    Ok(out)
}

/// A lambda or method reference (its target method) and a string concatenation (its text, `{}`
/// where values go).
fn read_indy(pool: &Pool, bootstrap: Option<&(u16, Vec<u16>)>, out: &mut Code) {
    let Some((handle, args)) = bootstrap else { return };
    let factory = match pool.get(*handle) {
        Some(Constant::MethodHandle { reference }) => pool.member(*reference),
        _ => None,
    };
    let Some(factory) = factory else { return };
    if factory.owner == "java/lang/invoke/StringConcatFactory" {
        if let Some(Constant::String(s)) = args.first().and_then(|a| pool.get(*a)) {
            let text = pool.utf8(*s).unwrap_or_default().replace('\u{1}', "{}");
            if !text.trim_matches(|c: char| c == '{' || c == '}' || c.is_whitespace()).is_empty()
                && !out.strings.contains(&text)
            {
                out.steps.push(Step::String(text.clone()));
                out.strings.push(text);
            }
        }
        return;
    }
    for a in args {
        if let Some(Constant::MethodHandle { reference }) = pool.get(*a) {
            if let Some(target) = pool.member(*reference) {
                out.steps.push(Step::Lambda(target.clone()));
                if !out.lambdas.contains(&target.name) {
                    out.lambdas.push(target.name.clone());
                }
                if !out.calls.contains(&target) {
                    out.calls.push(target);
                }
            }
        }
    }
}

/// The parameter types of a method descriptor, as internal names or primitive letters.
pub fn parameters(descriptor: &str) -> Vec<String> {
    let inner = descriptor.strip_prefix('(').and_then(|d| d.split(')').next()).unwrap_or("");
    let b = inner.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        while i < b.len() && b[i] == b'[' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i] == b'L' {
            let end = inner[i..].find(';').map_or(inner.len(), |e| i + e);
            out.push(format!("{}{}", &inner[start..i], &inner[i + 1..end]));
            i = end + 1;
        } else {
            out.push(inner[start..=i].to_owned());
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_split_into_parameters() {
        assert_eq!(
            parameters(
                "(Lorg/bukkit/command/CommandSender;Lorg/bukkit/command/Command;Ljava/lang/String;[Ljava/lang/String;)Z"
            ),
            ["org/bukkit/command/CommandSender", "org/bukkit/command/Command", "java/lang/String", "[java/lang/String"]
        );
        assert_eq!(parameters("(IJ[D)V"), ["I", "J", "[D"]);
        assert_eq!(parameters("()V"), Vec::<String>::new());
    }

    #[test]
    fn modified_utf8_reads_accents() {
        assert_eq!(modified_utf8("Dragão §a".as_bytes()), "Dragão §a");
    }
}
