//! What a plugin jar holds and does: its `plugin.yml`, listeners, commands, what runs when it is
//! enabled, and every Bukkit call each of those makes.

use crate::classfile::{self, Class, MemberRef, Method, Step};
use crate::mapping::{self, Port};
use horizon_plugin_api::PluginDescription;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::Path;

/// A file of the jar the plugin reads (`config.yml`, `messages.yml`, `lang/en.yml`).
pub struct Resource {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub struct Jar {
    pub classes: Vec<Class>,
    /// `plugin.yml`, or one made from `paper-plugin.yml`.
    pub plugin_yml: String,
    /// Whether `plugin.yml` came from `paper-plugin.yml`.
    pub from_paper_yml: bool,
    pub resources: Vec<Resource>,
    /// Class files that failed to read, with why.
    pub unreadable: Vec<(String, String)>,
    /// Jars inside the jar whose classes were read too.
    pub nested: Vec<String>,
}

pub fn read_jar(path: &Path) -> Result<Jar, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut found = Found::default();
    read_zip(&bytes, &mut found, 0).map_err(|e| format!("{}: {e}", path.display()))?;
    let Found { classes, unreadable, mut resources, plugin_yml, paper_yml, nested } = found;
    let (plugin_yml, from_paper_yml) = match (plugin_yml, paper_yml) {
        (Some(y), _) => (y, false),
        (None, Some(p)) => (from_paper_plugin_yml(&p), true),
        (None, None) => {
            return Err(format!("{}: no plugin.yml or paper-plugin.yml: not a Bukkit or Paper plugin", path.display()));
        }
    };
    resources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Jar { classes, plugin_yml, from_paper_yml, resources, unreadable, nested })
}

#[derive(Default)]
struct Found {
    classes: Vec<Class>,
    unreadable: Vec<(String, String)>,
    resources: Vec<Resource>,
    plugin_yml: Option<String>,
    paper_yml: Option<String>,
    nested: Vec<String>,
}

/// Reads a jar, and the jars inside it: some plugins (LuckPerms) keep their code in a jar their
/// loader opens (`*.jarinjar`). The outer jar's `plugin.yml` wins.
fn read_zip(bytes: &[u8], found: &mut Found, depth: u32) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a jar ({e})"))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| format!("{name}: {e}"))?;
        if name.ends_with(".class") {
            if name.starts_with("META-INF/") || name.ends_with("module-info.class") {
                continue;
            }
            match classfile::parse(&bytes) {
                Ok(c) => found.classes.push(c),
                Err(e) => found.unreadable.push((name, e)),
            }
        } else if (name.ends_with(".jarinjar") || name.ends_with(".jar")) && depth < 2 && !name.starts_with("META-INF/")
        {
            if read_zip(&bytes, found, depth + 1).is_ok() {
                found.nested.push(name);
            }
        } else if name == "plugin.yml" {
            if depth == 0 || found.plugin_yml.is_none() {
                found.plugin_yml = Some(String::from_utf8_lossy(&bytes).into_owned());
            }
        } else if name == "paper-plugin.yml" {
            if depth == 0 || found.paper_yml.is_none() {
                found.paper_yml = Some(String::from_utf8_lossy(&bytes).into_owned());
            }
        } else if is_resource(&name) && !found.resources.iter().any(|r| r.path == name) {
            found.resources.push(Resource { path: name, bytes });
        }
    }
    Ok(())
}

/// Files a plugin may read from its jar: texts of its own, not build leftovers or libraries'.
fn is_resource(name: &str) -> bool {
    if name.starts_with("META-INF/") || name == "bungee.yml" || name == "velocity-plugin.json" {
        return false;
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    matches!(ext, "yml" | "yaml" | "json" | "txt" | "properties" | "conf" | "toml")
        && !name.contains("/maven/")
        && !name.ends_with("pom.properties")
}

/// A `plugin.yml` with what `paper-plugin.yml` says that Bukkit's has: Paper's own lacks commands
/// and keeps dependencies as maps.
fn from_paper_plugin_yml(text: &str) -> String {
    let mut out =
        String::from("# Made from paper-plugin.yml: commands Paper plugins register in code are in src/lib.rs.\n");
    for line in text.lines() {
        let key = line.split(':').next().unwrap_or("");
        if !line.starts_with(' ')
            && matches!(
                key,
                "name"
                    | "version"
                    | "main"
                    | "description"
                    | "author"
                    | "authors"
                    | "website"
                    | "prefix"
                    | "load"
                    | "api-version"
            )
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The top-level `main:` of a `plugin.yml`.
pub fn main_class(plugin_yml: &str) -> Option<String> {
    plugin_yml.lines().find_map(|l| {
        let v = l.strip_prefix("main:")?.trim().trim_matches(['"', '\'']);
        (!v.is_empty()).then(|| v.replace('.', "/"))
    })
}

/// A Bukkit call a piece of code makes, and what it becomes.
#[derive(Clone, Debug)]
pub struct Use {
    pub call: MemberRef,
    pub port: Option<Port>,
}

/// What a piece of code (a method and the lambdas it holds) uses.
#[derive(Clone, Debug, Default)]
pub struct Uses {
    pub calls: Vec<Use>,
    /// Constants it reads (`Material.BEDROCK`) and how Horizon writes them.
    pub constants: Vec<(MemberRef, Option<String>)>,
    pub strings: Vec<String>,
}

impl Uses {
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty() && self.constants.is_empty() && self.strings.is_empty()
    }
}

pub struct Listener {
    pub class: String,
    pub method: String,
    /// Registered in code (`PluginManager.registerEvent`, as Kotlin plugins do) from `method`, not an
    /// `@EventHandler`: what it runs wasn't followed.
    pub dynamic: bool,
    /// The event's internal name.
    pub event: String,
    pub priority: String,
    pub ignore_cancelled: bool,
    pub uses: Uses,
}

pub struct Command {
    pub name: String,
    /// Declared in `plugin.yml` (a command only registered in code isn't).
    pub declared: bool,
    pub executor: Option<(String, Uses)>,
    pub completer: Option<(String, Uses)>,
}

/// One of the main class's `onEnable`, `onDisable`, `onLoad`.
pub struct Lifecycle {
    pub uses: Uses,
    pub calls_save_default_config: bool,
    /// Scheduler calls it makes, each with what its task does.
    pub tasks: Vec<(MemberRef, Uses)>,
}

pub struct Analysis {
    pub description: PluginDescription,
    pub plugin_yml: String,
    pub from_paper_yml: bool,
    pub main: String,
    pub on_enable: Option<Lifecycle>,
    pub on_disable: Option<Lifecycle>,
    pub on_load: Option<Lifecycle>,
    pub listeners: Vec<Listener>,
    pub commands: Vec<Command>,
    /// Other methods of the plugin's own classes that use the API, by class then method.
    pub other: Vec<(String, String, Uses)>,
    /// Packages bundled in the jar that aren't the plugin's (shaded libraries), with their class counts.
    pub libraries: Vec<(String, usize)>,
    pub resources: Vec<Resource>,
    pub unreadable: Vec<(String, String)>,
    /// Paper's code-registered commands (Brigadier, `LifecycleEvents.COMMANDS`) were seen.
    pub brigadier: bool,
    pub nested: Vec<String>,
    /// The plugin's own classes that reach into the server's internals (Minecraft's code, CraftBukkit),
    /// with how many such references each makes.
    pub internals: Vec<(String, usize)>,
}

/// Minecraft's own code and CraftBukkit's: what plugins reach with reflection or NMS. Nothing of it
/// carries over.
pub fn is_internal(owner: &str) -> bool {
    owner.starts_with("net/minecraft/")
        || owner.starts_with("org/bukkit/craftbukkit/")
        || owner.starts_with("com/mojang/")
}

pub const EVENT_HANDLER: &str = "Lorg/bukkit/event/EventHandler;";

struct Index<'a> {
    classes: HashMap<&'a str, &'a Class>,
}

impl<'a> Index<'a> {
    fn method(&self, class: &str, name: &str) -> Option<&'a Method> {
        self.classes.get(class)?.methods.iter().find(|m| m.name == name)
    }

    /// Whether `class` or one of its super classes (in the jar) implements `interface` or extends it.
    fn is_a(&self, class: &str, interface: &str) -> bool {
        let mut at = Some(class.to_owned());
        let mut guard = 0;
        while let Some(c) = at {
            if c == interface {
                return true;
            }
            let Some(k) = self.classes.get(c.as_str()) else { return false };
            if k.interfaces.iter().any(|i| i == interface || self.is_a(i, interface)) {
                return true;
            }
            at = k.super_name.clone();
            guard += 1;
            if guard > 32 {
                return false;
            }
        }
        false
    }

    /// What `method` of `class` uses, with its lambdas' code (and theirs) folded in.
    fn uses(&self, class: &str, method: &str) -> Uses {
        self.uses_except(class, method, &[])
    }

    /// As [`Index::uses`], leaving out the lambdas named in `skip` (tasks shown on their own).
    fn uses_except(&self, class: &str, method: &str, skip: &[String]) -> Uses {
        let mut out = Uses::default();
        let mut seen_calls = BTreeSet::new();
        let mut seen_constants = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut todo = vec![method.to_owned()];
        while let Some(m) = todo.pop() {
            if skip.contains(&m) || !visited.insert(m.clone()) {
                continue;
            }
            let Some(code) = self.method(class, &m).map(|m| &m.code) else { continue };
            for call in &code.calls {
                if call.owner == class && call.name.starts_with("lambda$") {
                    continue;
                }
                let port = mapping::call(&call.owner, &call.name);
                // Constructors of what the plugin extends (JavaPlugin, BukkitRunnable) are its own;
                // Object's methods on API types aren't the API.
                if (call.name == "<init>" && port.is_none())
                    || matches!(
                        call.name.as_str(),
                        "hashCode" | "getClass" | "toString" | "compareTo" | "ordinal" | "name" | "valueOf" | "values"
                    ) && !matches!(port, Some(Port::To(_)) | Some(Port::Missing(_)))
                {
                    continue;
                }
                if mapping::is_api(&call.owner)
                    && seen_calls.insert((mapping::simple_name(&call.owner).to_owned(), call.name.clone()))
                {
                    out.calls.push(Use { port, call: call.clone() });
                }
            }
            for f in &code.fields {
                if mapping::is_api(&f.owner) && seen_constants.insert(f.clone()) {
                    out.constants.push((f.clone(), mapping::constant(&f.owner, &f.name)));
                }
            }
            for s in &code.strings {
                if !s.trim().is_empty() && !out.strings.contains(s) {
                    out.strings.push(s.clone());
                }
            }
            // Lambdas and method references of this class run as part of it.
            for l in &code.lambdas {
                todo.push(l.clone());
            }
        }
        out
    }
}

pub fn analyze(jar: Jar) -> Result<Analysis, String> {
    let description = PluginDescription::from_yaml(&jar.plugin_yml).map_err(|e| format!("plugin.yml: {e}"))?;
    let main = main_class(&jar.plugin_yml).ok_or("plugin.yml has no `main`")?;
    let index = Index { classes: jar.classes.iter().map(|c| (c.name.as_str(), c)).collect() };
    if !index.classes.contains_key(main.as_str()) {
        return Err(format!("the main class {} isn't in the jar", main.replace('/', ".")));
    }
    let own_prefix = own_package(&main);
    let own = |c: &str| c.starts_with(&own_prefix) && relocated(c).is_none();

    // Listeners: every @EventHandler of the plugin's own classes.
    let mut listeners = Vec::new();
    for class in jar.classes.iter().filter(|c| own(&c.name)) {
        for m in &class.methods {
            let Some(a) = m.annotations.iter().find(|a| a.kind == EVENT_HANDLER) else { continue };
            let Some(event) = classfile::parameters(&m.descriptor).into_iter().next() else { continue };
            let value = |k: &str| a.values.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
            listeners.push(Listener {
                class: class.name.clone(),
                method: m.name.clone(),
                dynamic: false,
                event,
                priority: value("priority").unwrap_or_else(|| "NORMAL".into()),
                ignore_cancelled: value("ignoreCancelled").as_deref() == Some("true"),
                uses: index.uses(&class.name, &m.name),
            });
        }
    }

    // Listeners registered in code: `registerEvent(SomeEvent.class, ...)`.
    for class in jar.classes.iter().filter(|c| own(&c.name)) {
        for m in &class.methods {
            let mut event: Option<String> = None;
            for step in &m.code.steps {
                match step {
                    Step::ClassLiteral(c) if c.ends_with("Event") => event = Some(c.clone()),
                    Step::Call(c) if c.name == "registerEvent" && mapping::simple_name(&c.owner) == "PluginManager" => {
                        if let Some(e) = event.take() {
                            let method = if m.name.starts_with("lambda$") {
                                m.name.split('$').nth(1).unwrap_or(&m.name).to_owned()
                            } else {
                                m.name.clone()
                            };
                            listeners.push(Listener {
                                class: class.name.clone(),
                                method,
                                dynamic: true,
                                event: e,
                                priority: "NORMAL".into(),
                                ignore_cancelled: false,
                                uses: Uses::default(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Commands: what plugin.yml declares, and which class each `getCommand(name).setExecutor(x)` hands it to.
    let mut executors: BTreeMap<String, String> = BTreeMap::new();
    let mut completers: BTreeMap<String, String> = BTreeMap::new();
    let mut brigadier = false;
    for class in jar.classes.iter().filter(|c| own(&c.name)) {
        for m in &class.methods {
            let mut name: Option<String> = None;
            let mut last_string: Option<String> = None;
            let mut last_new: Option<String> = None;
            for step in &m.code.steps {
                match step {
                    Step::String(s) => last_string = Some(s.clone()),
                    Step::New(c) => last_new = Some(c.clone()),
                    Step::This | Step::ClassLiteral(_) => {}
                    Step::Call(c) if c.name == "getCommand" => {
                        name = last_string.take().map(|s| s.to_ascii_lowercase());
                        last_new = None;
                    }
                    Step::Call(c) if c.name == "setExecutor" || c.name == "setTabCompleter" => {
                        if let Some(n) = &name {
                            let by = last_new
                                .clone()
                                .filter(|k| index.classes.contains_key(k.as_str()))
                                .unwrap_or_else(|| class.name.clone());
                            let map = if c.name == "setExecutor" { &mut executors } else { &mut completers };
                            map.insert(n.clone(), by);
                        }
                    }
                    Step::Call(c)
                        if c.owner.contains("brigadier")
                            || c.owner.starts_with("io/papermc/paper/command/brigadier")
                            || c.owner.starts_with("io/papermc/paper/plugin/lifecycle") =>
                    {
                        brigadier = true;
                    }
                    Step::Call(_) => {}
                }
            }
        }
    }
    let main_handles =
        index.is_a(&main, "org/bukkit/command/CommandExecutor") || index.method(&main, "onCommand").is_some();
    let mut names: Vec<(String, bool)> = description.commands.iter().map(|c| (c.name.clone(), true)).collect();
    for n in executors.keys().chain(completers.keys()) {
        if !names.iter().any(|(x, _)| x == n) {
            names.push((n.clone(), false));
        }
    }
    let commands: Vec<Command> = names
        .into_iter()
        .map(|(name, declared)| {
            // JavaPlugin is itself the executor and completer of its commands until another is set.
            let executor = executors.get(&name).cloned().or_else(|| main_handles.then(|| main.clone()));
            let completer = completers.get(&name).cloned().or_else(|| {
                let by = executor.clone()?;
                index.method(&by, "onTabComplete").map(|_| by)
            });
            Command {
                executor: executor.map(|c| {
                    let u = index.uses(&c, "onCommand");
                    (c, u)
                }),
                completer: completer.map(|c| {
                    let u = index.uses(&c, "onTabComplete");
                    (c, u)
                }),
                name,
                declared,
            }
        })
        .collect();

    let lifecycle = |name: &str| -> Option<Lifecycle> {
        let m = index.method(&main, name)?;
        let calls_save_default_config = m.code.calls.iter().any(|c| c.name == "saveDefaultConfig");
        let mut tasks = Vec::new();
        let mut task_lambdas = Vec::new();
        // A scheduler call takes the lambda made just before it.
        let mut lambda: Option<String> = None;
        for step in &m.code.steps {
            if let Step::Call(c) = step {
                if c.owner == main && c.name.starts_with("lambda$") {
                    lambda = Some(c.name.clone());
                } else if matches!(mapping::simple_name(&c.owner), "BukkitScheduler" | "BukkitRunnable")
                    && (c.name.starts_with("run") || c.name.starts_with("schedule"))
                {
                    let uses = lambda
                        .take()
                        .map(|l| {
                            let u = index.uses(&main, &l);
                            task_lambdas.push(l);
                            u
                        })
                        .unwrap_or_default();
                    tasks.push((c.clone(), uses));
                }
            }
        }
        let mut uses = index.uses_except(&main, name, &task_lambdas);
        // Command names it hands to getCommand are the commands below, not text.
        uses.strings.retain(|s| !commands.iter().any(|c: &Command| c.name.eq_ignore_ascii_case(s)));
        Some(Lifecycle { uses, calls_save_default_config, tasks })
    };

    // Everything else of the plugin's own that uses the API.
    let mut covered: BTreeSet<(String, String)> = BTreeSet::new();
    for l in &listeners {
        covered.insert((l.class.clone(), l.method.clone()));
    }
    for (_, c) in executors.iter().chain(completers.iter()) {
        covered.insert((c.clone(), "onCommand".into()));
        covered.insert((c.clone(), "onTabComplete".into()));
    }
    if main_handles {
        covered.insert((main.clone(), "onCommand".into()));
        covered.insert((main.clone(), "onTabComplete".into()));
    }
    for m in ["onEnable", "onDisable", "onLoad"] {
        covered.insert((main.clone(), m.into()));
    }
    let mut other = Vec::new();
    for class in jar.classes.iter().filter(|c| own(&c.name)) {
        for m in &class.methods {
            if m.name.starts_with("lambda$") || covered.contains(&(class.name.clone(), m.name.clone())) {
                continue;
            }
            let uses = index.uses(&class.name, &m.name);
            if !uses.calls.is_empty() {
                other.push((class.name.clone(), m.name.clone(), uses));
            }
        }
    }

    let mut internals = Vec::new();
    for class in jar.classes.iter().filter(|c| own(&c.name)) {
        let n = class
            .methods
            .iter()
            .flat_map(|m| m.code.calls.iter().chain(&m.code.fields))
            .filter(|r| is_internal(&r.owner))
            .count();
        if n > 0 {
            internals.push((class.name.clone(), n));
        }
    }
    internals.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

    let mut libraries: BTreeMap<String, usize> = BTreeMap::new();
    for c in jar.classes.iter().filter(|c| !own(&c.name)) {
        *libraries.entry(relocated(&c.name).unwrap_or_else(|| library_name(&c.name))).or_default() += 1;
    }

    Ok(Analysis {
        on_enable: lifecycle("onEnable"),
        on_disable: lifecycle("onDisable"),
        on_load: lifecycle("onLoad"),
        description,
        plugin_yml: jar.plugin_yml,
        from_paper_yml: jar.from_paper_yml,
        main,
        listeners,
        commands,
        other,
        libraries: libraries.into_iter().collect(),
        resources: jar.resources,
        unreadable: jar.unreadable,
        brigadier,
        nested: jar.nested,
        internals,
    })
}

/// The plugin's own package: the main class's, up to its third part (`com/example/greeter/`), so
/// sub-packages count and shaded libraries (`org/bstats/`, `kotlin/`) don't.
fn own_package(main: &str) -> String {
    let parts: Vec<&str> = main.split('/').collect();
    let keep = (parts.len() - 1).min(3);
    let mut p = parts[..keep].join("/");
    if !p.is_empty() {
        p.push('/');
    }
    p
}

/// A library the build moved under the plugin's package (`com/x/plugin/libs/bstats/...`): its name
/// there (`com.x.plugin.libs.bstats`).
fn relocated(class: &str) -> Option<String> {
    let parts: Vec<&str> = class.split('/').collect();
    let at = parts[..parts.len().saturating_sub(1)].iter().position(|p| {
        matches!(
            *p,
            "libs"
                | "lib"
                | "shaded"
                | "shade"
                | "relocated"
                | "relocations"
                | "dependencies"
                | "deps"
                | "thirdparty"
                | "third_party"
                | "external"
        )
    })?;
    Some(parts[..(at + 2).min(parts.len() - 1)].join("."))
}

/// A shaded library's name: its first two package parts (`org.bstats`), three for long reversed
/// domains (`com.google.gson`).
fn library_name(class: &str) -> String {
    let parts: Vec<&str> = class.split('/').collect();
    let keep = if matches!(parts.first(), Some(&("com" | "org" | "net" | "io" | "me" | "dev"))) { 3 } else { 2 };
    parts[..keep.min(parts.len().saturating_sub(1)).max(1)].join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_class_is_read_from_plugin_yml() {
        assert_eq!(main_class("name: X\nmain: com.example.X\n").as_deref(), Some("com/example/X"));
        assert_eq!(main_class("main: 'a.B'").as_deref(), Some("a/B"));
    }

    #[test]
    fn own_classes_and_libraries_are_told_apart() {
        assert_eq!(own_package("com/example/greeter/Greeter"), "com/example/greeter/");
        assert_eq!(own_package("me/joao/Plugin"), "me/joao/");
        assert_eq!(library_name("org/bstats/bukkit/Metrics"), "org.bstats.bukkit");
        assert_eq!(library_name("kotlin/collections/ArraysKt"), "kotlin.collections");
        assert_eq!(relocated("com/x/axgraves/libs/axapi/nms/Foo").as_deref(), Some("com.x.axgraves.libs.axapi"));
        assert_eq!(relocated("com/x/axgraves/libs/Foo").as_deref(), Some("com.x.axgraves.libs"));
        assert_eq!(relocated("com/x/axgraves/commands/Foo"), None);
    }
}
