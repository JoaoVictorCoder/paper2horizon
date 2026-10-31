//! The crate a conversion writes: `Cargo.toml`, `plugin.yml`, the plugin's resources, `src/lib.rs`
//! with a listener for each `@EventHandler` and an executor for each command, and `PORTING.md`.

use crate::analysis::{Analysis, Command, Lifecycle, Listener, Task, Uses};
use crate::classfile::MemberRef;
use crate::mapping::{self, Port};
use crate::translate::{self, Helper, Kind, Param, Ret, Spec, Translated, ValueKind};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

/// Where the plugin API comes from in the crate's `Cargo.toml`.
pub enum ApiSource {
    Git,
    /// A local checkout of horizon-server-plugins' `api/`.
    Path(String),
}

pub const API_GIT: &str = "https://github.com/JoaoVictorCoder/horizon-server-plugins";

/// `com/example/Greeter` → `com.example.Greeter`.
pub fn java_name(internal: &str) -> String {
    internal.replace('/', ".")
}

/// The crate's name: the plugin's, lower case with dashes.
pub fn crate_name(plugin: &str) -> String {
    let mut out = String::new();
    for c in plugin.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) { format!("plugin-{out}") } else { out }
}

/// The plugin's type name in Rust.
pub fn struct_name(plugin: &str) -> String {
    let words: String = plugin
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect();
    if words.is_empty() || words.starts_with(|c: char| c.is_ascii_digit()) { format!("Plugin{words}") } else { words }
}

fn short(text: &str) -> String {
    let one_line = text.replace('\n', "\\n").replace('\r', "");
    if one_line.chars().count() > 90 { one_line.chars().take(87).collect::<String>() + "..." } else { one_line }
}

/// Comment lines saying what Java code calls and what each call becomes, `indent` spaces in.
fn notes(uses: &Uses, indent: usize, skip: &[&str]) -> Vec<String> {
    let pad = " ".repeat(indent);
    let mut out = Vec::new();
    let (internal, calls): (Vec<_>, Vec<_>) = uses
        .calls
        .iter()
        .filter(|u| !skip.contains(&u.call.name.as_str()))
        .partition(|u| crate::analysis::is_internal(&u.call.owner));
    if !calls.is_empty() {
        out.push(format!("{pad}// The Java code calls:"));
        let width =
            calls.iter().map(|u| mapping::simple_name(&u.call.owner).len() + u.call.name.len() + 1).max().unwrap_or(0);
        for u in calls {
            let what = format!("{}.{}", mapping::simple_name(&u.call.owner), u.call.name);
            let sender = matches!(mapping::simple_name(&u.call.owner), "CommandSender" | "ConsoleCommandSender");
            let line = match u.port {
                Some(Port::To(rust)) if sender => {
                    format!("{what:<width$} → {}", rust.replacen("player.", "sender.", 1))
                }
                Some(Port::To(rust)) => format!("{what:<width$} → {rust}"),
                Some(Port::Missing(why)) => format!("{what:<width$} ✗ {why}"),
                None => format!("{what:<width$} ? no Horizon equivalent known"),
            };
            out.push(format!("{pad}//   {line}"));
        }
    }
    if !internal.is_empty() {
        out.push(format!(
            "{pad}// ✗ And {} calls into the server's internals (Minecraft's code, CraftBukkit): no equivalent here.",
            internal.len()
        ));
    }
    let constants: Vec<String> = uses
        .constants
        .iter()
        .filter(|(f, _)| !crate::analysis::is_internal(&f.owner))
        .map(|(f, h)| match h {
            Some(h) => format!("{}.{} → {h}", mapping::simple_name(&f.owner), f.name),
            None => format!("{}.{}", mapping::simple_name(&f.owner), f.name),
        })
        .collect();
    if !constants.is_empty() {
        out.push(format!("{pad}// Constants: {}", constants.join(", ")));
    }
    for (i, s) in uses.strings.iter().take(12).enumerate() {
        let lead = if i == 0 { "// Text:" } else { "//      " };
        out.push(format!("{pad}{lead} \"{}\"", short(s)));
    }
    if uses.strings.len() > 12 {
        out.push(format!("{pad}//       ... and {} more", uses.strings.len() - 12));
    }
    out
}

fn priority(java: &str) -> String {
    mapping::camel(&java.to_ascii_lowercase())
}

/// Calls in `onEnable` that the generated `on_enable` already stands for.
const REGISTRATION: &[&str] = &[
    "registerEvents",
    "getPluginManager",
    "getCommand",
    "setExecutor",
    "setTabCompleter",
    "getServer",
    "getScheduler",
    "saveDefaultConfig",
    "runTask",
    "runTaskLater",
    "runTaskTimer",
    "runTaskAsynchronously",
    "runTaskLaterAsynchronously",
    "runTaskTimerAsynchronously",
    "scheduleSyncDelayedTask",
    "scheduleSyncRepeatingTask",
];

/// A translated body's place in `src/lib.rs` (1-based lines, both included), for the check that
/// takes back translations that don't compile.
#[derive(Clone, Debug)]
pub struct Region {
    pub id: String,
    pub start: usize,
    pub end: usize,
}

/// What became of each body the generator tried to translate.
#[derive(Clone, Debug)]
pub enum Outcome {
    Translated,
    /// Not translated, and why.
    Skipped(String),
    /// Translated, but it didn't compile: left as a comment.
    Broke,
}

pub struct Generated {
    pub lib_rs: String,
    pub regions: Vec<Region>,
    /// Each body by its id (`listener com.example.Greeter.onJoin`, `command heal`...).
    pub outcomes: Vec<(String, Outcome)>,
}

enum Attempt {
    Ok(Translated),
    Failed(String),
    Broke(Translated),
}

/// The plugin's own methods its translated code calls, each translated once into a Rust function.
struct Registry<'a> {
    a: &'a Analysis,
    has_config: bool,
    disabled: &'a BTreeSet<String>,
    entries: RefCell<BTreeMap<String, Entry>>,
    names: RefCell<BTreeSet<String>>,
}

enum Entry {
    InProgress,
    Done { id: String, java: String, helper: Helper, param_names: Vec<String>, body: Translated },
    Failed(String),
}

impl Registry<'_> {
    fn resolve(&self, m: &MemberRef) -> Result<Helper, String> {
        let key = format!("{}.{}{}", m.owner, m.name, m.descriptor);
        match self.entries.borrow().get(&key) {
            Some(Entry::Done { helper, .. }) => return Ok(helper.clone()),
            Some(Entry::InProgress) => return Err("which calls itself (recursion isn't translated)".into()),
            Some(Entry::Failed(why)) => return Err(why.clone()),
            None => {}
        }
        let Some(class) = self.a.class(&m.owner) else { return Err("not the plugin's own".into()) };
        let Some(method) =
            class.methods.iter().find(|x| x.name == m.name && x.descriptor == m.descriptor && x.body.is_some())
        else {
            return Err("not the plugin's own".into());
        };
        let id = format!("helper {}.{}", java_name(&m.owner), m.name);
        let fail = |why: String| {
            self.entries.borrow_mut().insert(key.clone(), Entry::Failed(why.clone()));
            Err(why)
        };
        let Some((params, ret)) = translate::helper_kinds(&m.descriptor) else {
            return fail("whose parameters or result have no Rust form here".into());
        };
        if self.disabled.contains(&id) {
            return fail("whose translation didn't compile".into());
        }
        self.entries.borrow_mut().insert(key.clone(), Entry::InProgress);
        let param_names = translate::param_names(method);
        let ret_spec = match &ret {
            None => Ret::Unit,
            Some(Kind::Bool) => Ret::Bool,
            Some(Kind::Str) => Ret::Value(ValueKind::Str),
            Some(Kind::I64) => Ret::Value(ValueKind::I64),
            Some(Kind::F64) => Ret::Value(ValueKind::F64),
            Some(_) => Ret::Value(ValueKind::I32),
        };
        let resolver = |m: &MemberRef| self.resolve(m);
        let result = translate::translate(&Spec {
            class,
            method,
            main: &self.a.main,
            fields: &self.a.fields,
            params: param_names.iter().zip(&params).map(|(n, k)| Param::Named(n.clone(), k.clone())).collect(),
            ret: ret_spec,
            has_config: self.has_config,
            command: None,
            helpers: Some(&resolver),
        });
        match result {
            Ok(body) => {
                let mut name = translate::snake(&m.name);
                let mut names = self.names.borrow_mut();
                if names.contains(&name) {
                    name = format!("{}_{name}", translate::snake(mapping::simple_name(&m.owner)));
                }
                while !names.insert(name.clone()) {
                    name.push('_');
                }
                let helper = Helper { name, params, ret };
                let java = format!("{}.{}", java_name(&m.owner), m.name);
                self.entries
                    .borrow_mut()
                    .insert(key, Entry::Done { id, java, helper: helper.clone(), param_names, body });
                Ok(helper)
            }
            Err(why) => fail(format!("which isn't translated ({why})")),
        }
    }
}

struct Gen<'a> {
    registry: &'a Registry<'a>,
    a: &'a Analysis,
    has_config: bool,
    disabled: &'a BTreeSet<String>,
    out: String,
    regions: Vec<Region>,
    outcomes: Vec<(String, Outcome)>,
}

/// A closure parameter's name: `_name` when the body doesn't use it.
fn param(uses: &BTreeSet<&str>, name: &str) -> String {
    if uses.contains(name) { name.to_owned() } else { format!("_{name}") }
}

impl Gen<'_> {
    fn line(&mut self, text: &str) {
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// The number of the next line written.
    fn next_line(&self) -> usize {
        self.out.matches('\n').count() + 1
    }

    fn attempt(
        &mut self,
        id: &str,
        class: &str,
        method: &str,
        params: Vec<Param>,
        ret: Ret,
        command: Option<&str>,
    ) -> Attempt {
        // The method, in the class or the first of its super classes (in the jar) that has it.
        let mut found = None;
        let mut at = Some(class.to_owned());
        for _ in 0..16 {
            let Some(c) = at.as_deref().and_then(|n| self.a.class(n)) else { break };
            if let Some(m) = c.method(method).filter(|m| m.body.is_some()) {
                found = Some((c, m));
                break;
            }
            at = c.super_name.clone();
        }
        let result = match found {
            None => Err(format!("{}.{method} isn't in the jar", java_name(class))),
            Some((class, method)) => {
                let registry = self.registry;
                let resolver = |m: &MemberRef| registry.resolve(m);
                translate::translate(&Spec {
                    class,
                    method,
                    main: &self.a.main,
                    fields: &self.a.fields,
                    params,
                    ret,
                    has_config: self.has_config,
                    command,
                    helpers: Some(&resolver),
                })
            }
        };
        let attempt = match result {
            Ok(t) if self.disabled.contains(id) => Attempt::Broke(t),
            Ok(t) => Attempt::Ok(t),
            Err(e) => Attempt::Failed(e),
        };
        let outcome = match &attempt {
            Attempt::Ok(_) => Outcome::Translated,
            Attempt::Failed(e) => Outcome::Skipped(e.clone()),
            Attempt::Broke(_) => Outcome::Broke,
        };
        self.outcomes.push((id.to_owned(), outcome));
        attempt
    }

    /// The parameters a closure's body uses.
    fn uses(attempt: &Attempt) -> BTreeSet<&'static str> {
        match attempt {
            Attempt::Ok(t) => t.uses.clone(),
            _ => BTreeSet::new(),
        }
    }

    /// Writes a closure: `open` (its first line, given the parameter names), the body or the TODO
    /// with `notes`, `fallback` (its value when not translated) and `close`.
    #[allow(clippy::too_many_arguments)]
    fn closure(
        &mut self,
        id: &str,
        attempt: Attempt,
        open: impl FnOnce(&BTreeSet<&'static str>) -> String,
        todo: &str,
        notes_lines: Vec<String>,
        fallback: Option<&str>,
        close: &str,
    ) {
        let uses = Self::uses(&attempt);
        let start = self.next_line();
        self.line(&open(&uses));
        match attempt {
            Attempt::Ok(t) => {
                for l in &t.lines {
                    self.line(&format!("            {l}"));
                }
                self.line(close);
                self.regions.push(Region { id: id.to_owned(), start, end: self.next_line() - 1 });
                return;
            }
            Attempt::Failed(why) => {
                self.line(&format!("            // TODO: {todo} Not translated: {why}."));
                for l in notes_lines {
                    self.line(&l);
                }
            }
            Attempt::Broke(t) => {
                self.line(&format!(
                    "            // TODO: {todo} Its translation didn't compile; here it is to finish:"
                ));
                for l in &t.lines {
                    self.line(&format!("            // {l}"));
                }
            }
        }
        if let Some(v) = fallback {
            self.line(&format!("            {v}"));
        }
        self.line(close);
    }

    fn listener(&mut self, l: &Listener) {
        let event = mapping::simple_name(&l.event);
        let from = if l.dynamic {
            format!(
                "registered in code by {}.{} (registerEvent): port what its executor runs",
                java_name(&l.class),
                l.method
            )
        } else {
            format!("{}.{}({event})", java_name(&l.class), l.method)
        };
        let Some((horizon, note)) = mapping::event(event).and_then(|m| m.horizon.map(|h| (h, m.note))) else {
            let why = mapping::event(event).map(|m| m.note).unwrap_or("No Horizon event yet.");
            self.line(&format!("        // {from}: left out, no Horizon event for {event}."));
            self.line(&format!("        // {why}"));
            for n in notes(&l.uses, 8, &[]) {
                self.line(&n);
            }
            self.line("");
            return;
        };
        self.line(&format!("        // {from}"));
        if !note.is_empty() {
            self.line(&format!("        // {note}"));
        }
        let id = format!("listener {}.{}", java_name(&l.class), l.method);
        let attempt = if l.dynamic {
            Attempt::Failed("it was registered in code".into())
        } else {
            self.attempt(&id, &l.class, &l.method, vec![Param::Event(horizon)], Ret::Unit, None)
        };
        let p = priority(&l.priority);
        let ignore = l.ignore_cancelled;
        let open = move |u: &BTreeSet<&'static str>| {
            let names = format!("{}, {}, {}", param(u, "plugin"), param(u, "ctx"), param(u, "e"));
            if p == "Normal" && !ignore {
                format!("        ctx.on(|{names}: &mut {horizon}| {{")
            } else {
                format!("        ctx.listen(EventPriority::{p}, {ignore}, |{names}: &mut {horizon}| {{")
            }
        };
        let todo = format!("port {}.", l.method);
        self.closure(&id, attempt, open, &todo, notes(&l.uses, 12, &[]), None, "        });");
        self.line("");
    }

    fn command(&mut self, c: &Command) {
        let name = c.name.clone();
        let from = match &c.executor {
            Some((class, _)) => format!("{}.{}", java_name(class), c.executor_method),
            None => "no executor found in the jar".to_owned(),
        };
        self.line(&format!("        // /{name}: {from}"));
        let id = format!("command {name}");
        let attempt = match &c.executor {
            Some((class, _)) => self.attempt(
                &id,
                class,
                &c.executor_method,
                vec![Param::Sender, Param::Command, Param::Label, Param::Args],
                Ret::Bool,
                Some(&name),
            ),
            None => Attempt::Failed("no executor".into()),
        };
        let declared = c.declared;
        if !declared {
            self.line("        // Not in plugin.yml: registered here (add it to plugin.yml to give it a usage and permission).");
        }
        let n = name.clone();
        let open = move |u: &BTreeSet<&'static str>| {
            let names = format!(
                "{}, {}, {}, {}, {}",
                param(u, "plugin"),
                param(u, "ctx"),
                param(u, "sender"),
                param(u, "label"),
                param(u, "args")
            );
            if declared {
                format!("        ctx.set_executor(\"{n}\", |{names}| {{")
            } else {
                format!("        ctx.command(CommandSpec::new(\"{n}\"), |{names}| {{")
            }
        };
        let notes_lines = c.executor.as_ref().map(|(_, u)| notes(u, 12, &[])).unwrap_or_default();
        self.closure(
            &id,
            attempt,
            open,
            "port the command (`sender.player()` is Some for a player).",
            notes_lines,
            Some("true // false shows the command's usage"),
            "        });",
        );
        if let Some((class, uses)) = &c.completer {
            self.line(&format!("        // {}.{}", java_name(class), c.completer_method));
            let id = format!("completer {name}");
            let attempt = self.attempt(
                &id,
                class,
                &c.completer_method,
                vec![Param::Sender, Param::Command, Param::Label, Param::Args],
                Ret::StrList,
                Some(&name),
            );
            let n = name.clone();
            let open = move |u: &BTreeSet<&'static str>| {
                let names = format!(
                    "{}, {}, {}, {}, {}",
                    param(u, "plugin"),
                    param(u, "ctx"),
                    param(u, "sender"),
                    param(u, "label"),
                    param(u, "args")
                );
                format!("        ctx.set_tab_completer(\"{n}\", |{names}| {{")
            };
            self.closure(
                &id,
                attempt,
                open,
                "port the completions (the last of `args` is the one being typed).",
                notes(uses, 12, &[]),
                Some("Vec::new()"),
                "        });",
            );
        }
        self.line("");
    }

    fn task(&mut self, t: &Task, n: usize) {
        let call = &t.call;
        let ticks = |i: usize| t.ticks.get(i).filter(|v| **v >= 0).map(|v| v.to_string());
        let (timing, todo_timing) = match call.name.as_str() {
            "runTaskLater" | "scheduleSyncDelayedTask" => match ticks(0) {
                Some(d) => (format!("ctx.run_later({d}, "), ""),
                None => ("ctx.run_later(20, ".to_owned(), " // TODO: the delay in ticks the Java code passes"),
            },
            "runTaskTimer" | "scheduleSyncRepeatingTask" => match (ticks(0), ticks(1)) {
                (Some(d), Some(p)) => (format!("ctx.run_timer({d}, {p}, "), ""),
                _ => (
                    "ctx.run_timer(20, 20, ".to_owned(),
                    " // TODO: the delay and period in ticks the Java code passes",
                ),
            },
            _ => ("ctx.run_task(".to_owned(), ""),
        };
        self.line(&format!("        // {}.{}", mapping::simple_name(&call.owner), call.name));
        if call.name.contains("Asynchronously") {
            self.line(
                "        ctx.run_async(|_sync| { // off the server thread: come back with _sync.run(|plugin, ctx| ...)",
            );
            self.line("            // TODO: port the task.");
            for l in notes(&t.uses, 12, &[]) {
                self.line(&l);
            }
            self.line("        });");
            self.line("");
            return;
        }
        let id = format!("task {n}");
        let attempt = match &t.lambda {
            Some(l) => self.attempt(&id, &self.a.main.clone(), l, Vec::new(), Ret::Unit, None),
            None => Attempt::Failed("the task isn't a lambda".into()),
        };
        let open = move |u: &BTreeSet<&'static str>| {
            format!("        {timing}|{}, {}| {{{todo_timing}", param(u, "plugin"), param(u, "ctx"))
        };
        self.closure(&id, attempt, open, "port the task.", notes(&t.uses, 12, &[]), None, "        });");
        self.line("");
    }

    fn lifecycle(&mut self, fn_name: &str, java: &str, l: &Lifecycle) {
        self.line(&format!("    fn {fn_name}(&mut self, ctx: &mut Ctx<Self>) {{"));
        let rest = notes(&l.uses, 8, REGISTRATION);
        if !rest.is_empty() {
            self.line(&format!("        // TODO: port the rest of {}.{java}.", java_name(&self.a.main)));
            for r in rest {
                self.line(&r);
            }
        }
        self.line("        let _ = ctx;");
        self.line("    }");
    }

    /// The helper functions the code written so far calls (and those they call).
    fn helpers(&mut self, plugin: &str) {
        let entries = std::mem::take(&mut *self.registry.entries.borrow_mut());
        let done: Vec<_> = entries
            .into_values()
            .filter_map(|e| match e {
                Entry::Done { id, java, helper, param_names, body } => Some((id, java, helper, param_names, body)),
                _ => None,
            })
            .collect();
        // Called from the plugin's code, or from a helper that is.
        let mut used: BTreeSet<usize> = BTreeSet::new();
        let calls = |text: &str, name: &str| text.contains(&format!("{name}(plugin, ctx"));
        loop {
            let before = used.len();
            for (i, (_, _, h, _, _)) in done.iter().enumerate() {
                if calls(&self.out, &h.name) || used.iter().any(|&j| done[j].4.lines.iter().any(|l| calls(l, &h.name)))
                {
                    used.insert(i);
                }
            }
            if used.len() == before {
                break;
            }
        }
        for (i, (id, java, helper, names, body)) in done.into_iter().enumerate() {
            if !used.contains(&i) {
                continue;
            }
            self.line("");
            self.line(&format!("/// Java: `{java}`."));
            let mut params = vec![
                format!("{}: &mut {plugin}", param(&body.uses, "plugin")),
                format!("{}: &mut Ctx<{plugin}>", param(&body.uses, "ctx")),
            ];
            for (n, k) in names.iter().zip(&helper.params) {
                let used = body.lines.iter().any(|l| translate::uses_ident(l, n));
                params.push(format!("{}{n}: {}", if used { "" } else { "_" }, translate::param_type(k)));
            }
            let ret = helper.ret.as_ref().map(|k| format!(" -> {}", translate::param_type(k))).unwrap_or_default();
            let start = self.next_line();
            self.line(&format!("fn {}({}){ret} {{", helper.name, params.join(", ")));
            for l in &body.lines {
                self.line(&format!("    {l}"));
            }
            self.line("}");
            self.regions.push(Region { id: id.clone(), start, end: self.next_line() - 1 });
            self.outcomes.push((id, Outcome::Translated));
        }
    }

    fn trim_blank(&mut self) {
        while self.out.ends_with("\n\n") {
            self.out.pop();
        }
    }

    fn plugin_struct(&mut self, name: &str) {
        let a = self.a;
        self.line(&format!("/// The plugin (Java: {}).", java_name(&a.main)));
        let fields: Vec<_> = a.fields.iter().collect();
        let custom = fields.iter().any(|(_, f)| f.init.is_some());
        if !custom {
            self.line("#[derive(Default)]");
        }
        if fields.is_empty() {
            self.line(&format!("pub struct {name} {{}}"));
        } else {
            self.line(&format!("pub struct {name} {{"));
            for (java, f) in &fields {
                if java.as_str() != f.rust {
                    self.line(&format!("    /// Java: `{java}`."));
                }
                self.line(&format!("    pub {}: {},", f.rust, f.ty));
            }
            self.line("}");
        }
        if custom {
            self.line("");
            self.line(&format!("impl Default for {name} {{"));
            self.line("    fn default() -> Self {");
            self.line(&format!("        {name} {{"));
            for (_, f) in &fields {
                match &f.init {
                    Some(v) => self.line(&format!("            {}: {v},", f.rust)),
                    None => self.line(&format!("            {}: Default::default(),", f.rust)),
                }
            }
            self.line("        }");
            self.line("    }");
            self.line("}");
        }
        self.line("");
    }
}

/// `src/lib.rs`, with the bodies whose ids are in `disabled` left as comments.
pub fn lib_rs(a: &Analysis, has_config: bool, disabled: &BTreeSet<String>) -> Generated {
    let name = struct_name(&a.description.name);
    // Helpers keep clear of the prelude's names (`color::translate`, `server()`).
    let taken = ["color", "server", "worlds", "plugin_yml", "horizon_plugin"].map(String::from).into_iter().collect();
    let registry = Registry { a, has_config, disabled, entries: RefCell::default(), names: RefCell::new(taken) };
    let mut g = Gen {
        registry: &registry,
        a,
        has_config,
        disabled,
        out: String::new(),
        regions: Vec::new(),
        outcomes: Vec::new(),
    };
    g.line(&format!(
        "//! {} {}, converted from its Paper plugin by paper2horizon.",
        a.description.name, a.description.version
    ));
    g.line("//!");
    g.line("//! Each listener, command and task stands where the Java one was. Those paper2horizon could translate");
    g.line(
        "//! are Rust already; the others hold a TODO with what the Java code called, mapped to horizon-plugin-api.",
    );
    g.line("//! PORTING.md has the whole list.");
    g.line("");
    g.line("use horizon_plugin_api::prelude::*;");
    g.line("");
    g.plugin_struct(&name);
    g.line(&format!("impl Plugin for {name} {{"));
    if let Some(l) = &a.on_load {
        g.lifecycle("on_load", "onLoad", l);
        g.line("");
    }
    g.line("    fn on_enable(&mut self, ctx: &mut Ctx<Self>) {");
    if let Some(l) = &a.on_enable {
        if l.calls_save_default_config && has_config {
            g.line("        ctx.save_default_config(include_str!(\"../config.yml\"));");
            g.line("");
        }
        let rest = notes(&l.uses, 8, REGISTRATION);
        if !rest.is_empty() {
            g.line(&format!("        // TODO: port the rest of {}.onEnable.", java_name(&a.main)));
            for r in rest {
                g.line(&r);
            }
            g.line("");
        }
        for (i, t) in l.tasks.iter().enumerate() {
            g.task(t, i + 1);
        }
    }
    for l in &a.listeners {
        g.listener(l);
    }
    for c in &a.commands {
        g.command(c);
    }
    if a.brigadier {
        g.line("        // TODO: the plugin registers Brigadier commands (Paper's LifecycleEvents.COMMANDS): add each with");
        g.line("        // ctx.command(CommandSpec::new(\"name\"), ...) and ctx.set_tab_completer.");
        g.line("");
    }
    g.trim_blank();
    g.line("    }");
    if let Some(l) = &a.on_disable {
        g.line("");
        g.lifecycle("on_disable", "onDisable", l);
    }
    g.line("}");
    g.helpers(&name);
    g.line("");
    g.line(&format!("horizon_plugin!({name}::default(), plugin_yml!());"));
    Generated { lib_rs: g.out, regions: g.regions, outcomes: g.outcomes }
}

pub fn cargo_toml(a: &Analysis, api: &ApiSource) -> String {
    let dep = match api {
        ApiSource::Git => format!("horizon-plugin-api = {{ git = \"{API_GIT}\" }}"),
        ApiSource::Path(p) => format!("horizon-plugin-api = {{ path = \"{p}\" }}"),
    };
    let version = {
        // Cargo wants semver: keep the digits of the plugin's version, padded to three parts.
        let parts: Vec<String> = a
            .description
            .version
            .split(['.', '-', '+', ' '])
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .filter(|p| !p.is_empty())
            .take(3)
            .collect();
        let mut parts = parts;
        while parts.len() < 3 {
            parts.push("0".into());
        }
        parts.join(".")
    };
    format!(
        "[package]\nname = \"{}\"\nversion = \"{version}\"\nedition = \"2024\"\ndescription = \"{}\"\n\n\
         # The library a Horizon server loads from plugins/.\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n{dep}\n",
        crate_name(&a.description.name),
        a.description.description.replace('"', "'").replace('\n', " "),
    )
}

pub fn porting_md(a: &Analysis, jar: &str, generated: &Generated) -> String {
    let d = &a.description;
    let mut out = String::new();
    let _ = writeln!(out, "# Porting {} {} to Horizon\n", d.name, d.version);
    let _ = writeln!(
        out,
        "Made by paper2horizon from `{jar}`. Paper plugins are Java; this crate is the same plugin in Rust for \
         [horizon-plugin-api]({API_GIT}): `plugin.yml`, a listener for each `@EventHandler`, an executor for each \
         command, the tasks `onEnable` schedules. The bodies paper2horizon could translate are Rust already (read \
         them over: Java's `null` checks and corner cases may differ). The others are a `TODO` listing what the Java \
         code called and what that is here. Port them one by one, then build:\n"
    );
    let _ = writeln!(
        out,
        "```sh\ncargo build --release\ncp target/release/lib{}.so /path/to/server/plugins/\n```\n",
        crate_name(&d.name).replace('-', "_")
    );

    let mapped_listeners = a
        .listeners
        .iter()
        .filter(|l| mapping::event(mapping::simple_name(&l.event)).and_then(|m| m.horizon).is_some())
        .count();
    let all_uses: Vec<&Uses> = a
        .listeners
        .iter()
        .map(|l| &l.uses)
        .chain(a.commands.iter().filter_map(|c| c.executor.as_ref().map(|(_, u)| u)))
        .chain(a.commands.iter().filter_map(|c| c.completer.as_ref().map(|(_, u)| u)))
        .chain([&a.on_enable, &a.on_disable, &a.on_load].into_iter().flatten().map(|l| &l.uses))
        .chain(a.other.iter().map(|(_, _, u)| u))
        .collect();
    let mut distinct = std::collections::BTreeMap::new();
    for u in &all_uses {
        for c in u.calls.iter().filter(|c| !crate::analysis::is_internal(&c.call.owner)) {
            distinct.insert((mapping::simple_name(&c.call.owner).to_owned(), c.call.name.clone()), c.port);
        }
    }
    let to = distinct.values().filter(|p| matches!(p, Some(Port::To(_)))).count();
    let missing = distinct.values().filter(|p| matches!(p, Some(Port::Missing(_)))).count();
    let unknown = distinct.values().filter(|p| p.is_none()).count();

    let _ = writeln!(out, "## Summary\n");
    let _ = writeln!(out, "| | |\n|---|---|");
    let _ = writeln!(out, "| Listeners | {} ({mapped_listeners} with a Horizon event) |", a.listeners.len());
    let _ = writeln!(
        out,
        "| Commands | {} |",
        if a.commands.is_empty() {
            "none".to_owned()
        } else {
            a.commands.iter().map(|c| format!("`/{}`", c.name)).collect::<Vec<_>>().join(", ")
        }
    );
    let tasks = a.on_enable.as_ref().map_or(0, |l| l.tasks.len());
    let translated = generated.outcomes.iter().filter(|(_, o)| matches!(o, Outcome::Translated)).count();
    let _ = writeln!(
        out,
        "| Translated to Rust | {translated} of {} listeners, commands, completers, tasks and helpers |",
        generated.outcomes.len()
    );
    let _ = writeln!(out, "| Tasks scheduled in `onEnable` | {tasks} |");
    let _ = writeln!(
        out,
        "| Bukkit/Paper calls | {} different: {to} with an equivalent, {missing} missing in Horizon, {unknown} unknown |",
        distinct.len()
    );
    if !a.internals.is_empty() {
        let total: usize = a.internals.iter().map(|(_, n)| n).sum();
        let _ = writeln!(
            out,
            "| Server internals (NMS, CraftBukkit) | {total} references in {} classes: no equivalent |",
            a.internals.len()
        );
    }
    if !a.libraries.is_empty() {
        let libs: Vec<String> = a.libraries.iter().map(|(n, c)| format!("`{n}` ({c})")).collect();
        let _ = writeln!(out, "| Libraries bundled in the jar | {} |", libs.join(", "));
    }
    if !a.resources.is_empty() {
        let res: Vec<String> = a.resources.iter().map(|r| format!("`{}`", r.path)).collect();
        let _ = writeln!(out, "| Resources | {} |", res.join(", "));
    }
    out.push('\n');

    let deps: Vec<&String> = d.depend.iter().chain(&d.soft_depend).collect();
    if !deps.is_empty()
        || a.from_paper_yml
        || a.brigadier
        || !a.libraries.is_empty()
        || !a.unreadable.is_empty()
        || !a.internals.is_empty()
        || !a.nested.is_empty()
    {
        let _ = writeln!(out, "## Before you start\n");
        if !a.internals.is_empty() {
            let total: usize = a.internals.iter().map(|(_, n)| n).sum();
            let top: Vec<String> =
                a.internals.iter().take(5).map(|(c, n)| format!("`{}` ({n})", java_name(c))).collect();
            let _ = writeln!(
                out,
                "- **It reaches into the server's internals** (Minecraft's own code or CraftBukkit, {total} references in {} classes: {}). \
                 None of that carries over: Horizon is a different server. Those parts need the API, or a change to Horizon itself.",
                a.internals.len(),
                top.join(", ")
            );
        }
        if !a.nested.is_empty() {
            let _ = writeln!(out, "- Its code is in jars inside the jar ({}); those were read too.", list(&a.nested));
        }
        if !d.depend.is_empty() {
            let _ = writeln!(
                out,
                "- It needs {}: each must be a Horizon plugin too, or its part done another way.",
                list(&d.depend)
            );
        }
        if !d.soft_depend.is_empty() {
            let _ = writeln!(out, "- It works with {} when they are there (`softdepend`).", list(&d.soft_depend));
        }
        if a.from_paper_yml {
            let _ = writeln!(
                out,
                "- The jar has `paper-plugin.yml`, no `plugin.yml`: the one here keeps its name, version and description. Add `commands:` for the commands it registers in code."
            );
        }
        if a.brigadier {
            let _ = writeln!(
                out,
                "- It registers commands with Brigadier (Paper's `LifecycleEvents.COMMANDS`): add them with `ctx.command` and `ctx.set_tab_completer`."
            );
        }
        if !a.libraries.is_empty() {
            let _ = writeln!(
                out,
                "- Libraries bundled in the jar are Java: use Rust crates for what they did (bStats has no Horizon side; Gson → serde_json; HikariCP/JDBC → sqlx or rusqlite)."
            );
        }
        for (c, e) in &a.unreadable {
            let _ = writeln!(out, "- `{c}` couldn't be read: {e}.");
        }
        out.push('\n');
    }

    let left: Vec<_> = generated.outcomes.iter().filter(|(_, o)| !matches!(o, Outcome::Translated)).collect();
    if !left.is_empty() {
        let _ = writeln!(out, "## Not translated\n");
        let _ = writeln!(out, "These are `TODO`s in `src/lib.rs`, with what their Java code called:\n");
        for (id, o) in left {
            let why = match o {
                Outcome::Skipped(why) => why.clone(),
                Outcome::Broke => "its translation didn't compile (it is in src/lib.rs as a comment, to finish)".into(),
                Outcome::Translated => unreachable!(),
            };
            let _ = writeln!(out, "- `{id}`: {why}.");
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Listeners\n");
    if a.listeners.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(out, "| Java | Event | Horizon | Priority |\n|---|---|---|---|");
        for l in &a.listeners {
            let e = mapping::simple_name(&l.event);
            let h = match mapping::event(e) {
                Some(m) => match m.horizon {
                    Some(h) if m.note.is_empty() => format!("`{h}`"),
                    Some(h) => format!("`{h}`: {}", m.note),
                    None => format!("✗ {}", m.note),
                },
                None => "✗ No Horizon event yet.".to_owned(),
            };
            let ignore = if l.ignore_cancelled {
                ", ignoreCancelled"
            } else if l.dynamic {
                " (registerEvent)"
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "| `{}.{}` | `{e}` | {h} | {}{ignore} |",
                mapping::simple_name(&l.class),
                l.method,
                l.priority.to_ascii_lowercase()
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Commands\n");
    if a.commands.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(out, "| Command | Executor | Tab completion |\n|---|---|---|");
        for c in &a.commands {
            let ex = c.executor.as_ref().map_or("✗ not found".to_owned(), |(k, _)| format!("`{}`", java_name(k)));
            let tab = c
                .completer
                .as_ref()
                .map_or("players' names (Horizon's default)".to_owned(), |(k, _)| format!("`{}`", java_name(k)));
            let declared = if c.declared { "" } else { " (registered in code, not in plugin.yml)" };
            let _ = writeln!(out, "| `/{}`{declared} | {ex} | {tab} |", c.name);
        }
        out.push('\n');
    }

    let missing_list: Vec<_> = distinct
        .iter()
        .filter_map(|(k, p)| match p {
            Some(Port::Missing(why)) => Some((k, *why)),
            _ => None,
        })
        .collect();
    if !missing_list.is_empty() {
        let _ = writeln!(out, "## Missing in Horizon\n");
        let _ =
            writeln!(out, "What the plugin calls that horizon-plugin-api doesn't have yet, and what comes closest:\n");
        for ((owner, name), why) in missing_list {
            let _ = writeln!(out, "- `{owner}.{name}`: {why}");
        }
        out.push('\n');
    }
    let unknown_list: Vec<_> = distinct.iter().filter(|(_, p)| p.is_none()).map(|(k, _)| k).collect();
    if !unknown_list.is_empty() {
        let _ = writeln!(out, "## Calls with no known equivalent\n");
        let _ = writeln!(
            out,
            "Not in paper2horizon's tables. Look for them in the API's docs (docs/plugins.md), or do without:\n"
        );
        let _ = writeln!(
            out,
            "{}\n",
            unknown_list.iter().map(|(o, n)| format!("`{o}.{n}`")).collect::<Vec<_>>().join(", ")
        );
    }

    if !a.other.is_empty() {
        let _ = writeln!(out, "## Other code that uses the API\n");
        let _ = writeln!(
            out,
            "Methods outside the listeners, commands and `onEnable`, to port where they are called from:\n"
        );
        let shown: Vec<_> = a
            .other
            .iter()
            .filter(|(_, _, u)| u.calls.iter().any(|c| !crate::analysis::is_internal(&c.call.owner)))
            .collect();
        if shown.len() > 60 {
            let _ = writeln!(out, "The first 60 of {}.\n", shown.len());
        }
        for (class, method, uses) in shown.into_iter().take(60) {
            let _ = writeln!(out, "### `{}.{method}`\n", java_name(class));
            let _ = writeln!(out, "```text");
            for l in notes(uses, 0, &[]) {
                let _ = writeln!(out, "{}", l.trim_start_matches("// "));
            }
            let _ = writeln!(out, "```\n");
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn list(names: &[String]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

/// Writes the crate into `dir`.
pub fn write_crate(
    a: &Analysis,
    jar: &str,
    dir: &Path,
    api: &ApiSource,
    disabled: &BTreeSet<String>,
) -> Result<(Vec<String>, Generated), String> {
    let mut written = Vec::new();
    let mut write = |rel: &str, bytes: &[u8]| -> Result<(), String> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(rel.to_owned());
        Ok(())
    };
    let has_config = a.resources.iter().any(|r| r.path == "config.yml");
    write("Cargo.toml", cargo_toml(a, api).as_bytes())?;
    write(".gitignore", b"/target\n")?;
    write("plugin.yml", a.plugin_yml.as_bytes())?;
    for r in &a.resources {
        // config.yml beside Cargo.toml, for include_str!("../config.yml"); the others under resources/.
        let rel = if r.path == "config.yml" { "config.yml".to_owned() } else { format!("resources/{}", r.path) };
        write(&rel, &r.bytes)?;
    }
    let generated = lib_rs(a, has_config, disabled);
    write("src/lib.rs", generated.lib_rs.as_bytes())?;
    write("PORTING.md", porting_md(a, jar, &generated).as_bytes())?;
    Ok((written, generated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_rust_names() {
        assert_eq!(crate_name("Greeter"), "greeter");
        assert_eq!(crate_name("My Cool_Plugin!"), "my-cool-plugin");
        assert_eq!(crate_name("123"), "plugin-123");
        assert_eq!(struct_name("my-cool plugin"), "MyCoolPlugin");
        assert_eq!(struct_name("9lives"), "Plugin9lives");
    }
}
