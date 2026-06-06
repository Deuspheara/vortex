use std::collections::HashMap;
use std::sync::Arc;

use regex::RegexBuilder;

use crate::fs::{VirtualDirEntry, VirtualFs};
use crate::shell::ensure_writes_allowed;
use crate::{ExecutionLimits, ShellError, ShellPolicy, ShellResult, VirtualPath};

pub struct CommandContext {
    pub fs: Arc<dyn VirtualFs>,
    pub cwd: VirtualPath,
    pub policy: ShellPolicy,
    pub limits: ExecutionLimits,
}

pub trait BuiltinCommand: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl CommandOutput {
    fn stdout(text: impl Into<String>) -> Self {
        Self {
            stdout: text.into(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn code(exit_code: i32) -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            exit_code,
        }
    }
}

#[derive(Default)]
pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn BuiltinCommand>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Pwd);
        registry.register(Echo);
        registry.register(Ls);
        registry.register(Cat);
        registry.register(Head);
        registry.register(Tail);
        registry.register(Wc);
        registry.register(Grep);
        registry.register(Find);
        registry.register(Tree);
        registry.register(Sed);
        registry.register(Test);
        registry.register(Mkdir);
        registry.register(Touch);
        registry.register(Rm);
        registry.register(Cp);
        registry.register(Mv);
        registry
    }

    pub fn register(&mut self, command: impl BuiltinCommand + 'static) {
        self.commands.insert(
            command.name().into(),
            Arc::new(command) as Arc<dyn BuiltinCommand>,
        );
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn BuiltinCommand>> {
        self.commands.get(name).cloned()
    }
}

fn path(ctx: &CommandContext, arg: Option<&String>) -> ShellResult<VirtualPath> {
    VirtualPath::normalize(&ctx.cwd, arg.map(String::as_str).unwrap_or("."))
}

fn read_text(ctx: &CommandContext, path: &VirtualPath) -> ShellResult<String> {
    let data = ctx.fs.read_file(path)?;
    if data.len() > ctx.limits.max_file_read_bytes || data.len() > ctx.policy.max_file_read_bytes {
        return Err(ShellError::LimitExceeded(format!(
            "{path}: file exceeds max read size"
        )));
    }
    Ok(String::from_utf8_lossy(&data).into_owned())
}

fn join_lines(lines: impl IntoIterator<Item = String>) -> String {
    let mut out = lines.into_iter().collect::<Vec<_>>().join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

struct Pwd;
impl BuiltinCommand for Pwd {
    fn name(&self) -> &'static str {
        "pwd"
    }

    fn run(&self, ctx: &mut CommandContext, _args: &[String]) -> ShellResult<CommandOutput> {
        Ok(CommandOutput::stdout(format!("{}\n", ctx.cwd)))
    }
}

struct Echo;
impl BuiltinCommand for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn run(&self, _ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        Ok(CommandOutput::stdout(format!("{}\n", args.join(" "))))
    }
}

struct Ls;
impl BuiltinCommand for Ls {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let mut show_all = false;
        let mut long = false;
        let mut paths = Vec::new();
        for arg in args {
            if arg.starts_with('-') {
                show_all |= arg.contains('a');
                long |= arg.contains('l');
            } else {
                paths.push(arg);
            }
        }
        let target = path(ctx, paths.first().copied())?;
        let meta = ctx.fs.metadata(&target)?;
        if meta.is_file {
            return Ok(CommandOutput::stdout(format!("{}\n", target.name())));
        }
        let mut entries = ctx.fs.list_dir(&target)?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let lines = entries
            .into_iter()
            .filter(|entry| show_all || !entry.name.starts_with('.'))
            .map(|entry| {
                if long {
                    format!(
                        "{} {:>8} {}",
                        if entry.metadata.is_dir { "d" } else { "-" },
                        entry.metadata.len,
                        entry.name
                    )
                } else {
                    entry.name
                }
            });
        Ok(CommandOutput::stdout(join_lines(lines)))
    }
}

struct Cat;
impl BuiltinCommand for Cat {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        if args.is_empty() {
            return Err(ShellError::InvalidInput("cat: missing file".into()));
        }
        let mut out = String::new();
        for arg in args {
            out.push_str(&read_text(ctx, &path(ctx, Some(arg))?)?);
        }
        Ok(CommandOutput::stdout(out))
    }
}

struct Head;
impl BuiltinCommand for Head {
    fn name(&self) -> &'static str {
        "head"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let (n, files) = count_flag(args, 10)?;
        let file = files
            .first()
            .ok_or_else(|| ShellError::InvalidInput("head: missing file".into()))?;
        let text = read_text(ctx, &path(ctx, Some(file))?)?;
        Ok(CommandOutput::stdout(join_lines(
            text.lines().take(n).map(str::to_string),
        )))
    }
}

struct Tail;
impl BuiltinCommand for Tail {
    fn name(&self) -> &'static str {
        "tail"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let (n, files) = count_flag(args, 10)?;
        let file = files
            .first()
            .ok_or_else(|| ShellError::InvalidInput("tail: missing file".into()))?;
        let text = read_text(ctx, &path(ctx, Some(file))?)?;
        let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
        Ok(CommandOutput::stdout(join_lines(
            lines
                .into_iter()
                .rev()
                .take(n)
                .collect::<Vec<_>>()
                .into_iter()
                .rev(),
        )))
    }
}

fn count_flag(args: &[String], default: usize) -> ShellResult<(usize, Vec<&String>)> {
    let mut n = default;
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-n" {
            i += 1;
            n = args
                .get(i)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| ShellError::InvalidInput("-n requires a number".into()))?;
        } else if let Some(rest) = args[i].strip_prefix("-n") {
            n = rest
                .parse()
                .map_err(|_| ShellError::InvalidInput("-n requires a number".into()))?;
        } else {
            files.push(&args[i]);
        }
        i += 1;
    }
    Ok((n, files))
}

struct Wc;
impl BuiltinCommand for Wc {
    fn name(&self) -> &'static str {
        "wc"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let mut flags = Vec::new();
        let mut files = Vec::new();
        for arg in args {
            if arg.starts_with('-') {
                flags.extend(arg.trim_start_matches('-').chars());
            } else {
                files.push(arg);
            }
        }
        if flags.is_empty() {
            flags.extend(['l', 'w', 'c']);
        }
        let file = files
            .first()
            .ok_or_else(|| ShellError::InvalidInput("wc: missing file".into()))?;
        let text = read_text(ctx, &path(ctx, Some(file))?)?;
        let mut parts = Vec::new();
        for flag in flags {
            match flag {
                'l' => parts.push(text.lines().count().to_string()),
                'w' => parts.push(text.split_whitespace().count().to_string()),
                'c' => parts.push(text.len().to_string()),
                _ => {}
            }
        }
        parts.push(file.to_string());
        Ok(CommandOutput::stdout(format!("{}\n", parts.join(" "))))
    }
}

struct Grep;
impl BuiltinCommand for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let mut recursive = false;
        let mut line_numbers = false;
        let mut insensitive = false;
        let mut rest = Vec::new();
        for arg in args {
            if arg.starts_with('-') {
                recursive |= arg.contains('R') || arg.contains('r');
                line_numbers |= arg.contains('n');
                insensitive |= arg.contains('i');
            } else {
                rest.push(arg);
            }
        }
        let pattern = rest
            .first()
            .ok_or_else(|| ShellError::InvalidInput("grep: missing pattern".into()))?;
        let targets = if rest.len() > 1 {
            rest[1..]
                .iter()
                .map(|s| path(ctx, Some(s)))
                .collect::<ShellResult<Vec<_>>>()?
        } else {
            vec![ctx.cwd.clone()]
        };
        let regex = RegexBuilder::new(pattern)
            .case_insensitive(insensitive)
            .build()
            .map_err(|e| ShellError::InvalidInput(format!("grep: {e}")))?;
        let mut files = Vec::new();
        for target in targets {
            collect_files(ctx, &target, recursive, 0, &mut files)?;
        }
        let mut lines = Vec::new();
        for file in files {
            let text = read_text(ctx, &file)?;
            for (ix, line) in text.lines().enumerate() {
                if regex.is_match(line) {
                    if line_numbers {
                        lines.push(format!("{file}:{}:{line}", ix + 1));
                    } else {
                        lines.push(format!("{file}:{line}"));
                    }
                    if lines.len() >= ctx.limits.max_grep_matches {
                        lines.push("[grep matches truncated]".into());
                        return Ok(CommandOutput::stdout(join_lines(lines)));
                    }
                }
            }
        }
        Ok(CommandOutput::stdout(join_lines(lines)))
    }
}

fn collect_files(
    ctx: &CommandContext,
    target: &VirtualPath,
    recursive: bool,
    depth: usize,
    files: &mut Vec<VirtualPath>,
) -> ShellResult<()> {
    let meta = ctx.fs.metadata(target)?;
    if meta.is_file {
        files.push(target.clone());
        return Ok(());
    }
    if !recursive {
        return Err(ShellError::IsDirectory(format!("{target}: is a directory")));
    }
    if depth >= ctx.limits.max_recursion_depth {
        return Ok(());
    }
    if files.len() >= ctx.limits.max_traversal_entries {
        return Ok(());
    }
    for entry in ctx.fs.list_dir(target)? {
        if entry.metadata.is_file {
            files.push(entry.path);
        } else if entry.metadata.is_dir {
            collect_files(ctx, &entry.path, true, depth + 1, files)?;
        }
        if files.len() >= ctx.limits.max_traversal_entries {
            break;
        }
    }
    Ok(())
}

struct Find;
impl BuiltinCommand for Find {
    fn name(&self) -> &'static str {
        "find"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let mut root_arg: Option<&String> = None;
        let mut name_filter: Option<String> = None;
        let mut max_depth = ctx.limits.max_recursion_depth;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-name" => {
                    i += 1;
                    name_filter = args.get(i).cloned();
                }
                "-maxdepth" => {
                    i += 1;
                    max_depth = args
                        .get(i)
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| ShellError::InvalidInput("find: invalid maxdepth".into()))?;
                }
                arg if !arg.starts_with('-') && root_arg.is_none() => root_arg = Some(&args[i]),
                _ => {}
            }
            i += 1;
        }
        let root = path(ctx, root_arg)?;
        let mut out = Vec::new();
        walk(ctx, &root, 0, max_depth, &mut out)?;
        if let Some(filter) = name_filter {
            out.retain(|p| glob_match(&filter, p.name()));
        }
        if out.len() > ctx.limits.max_traversal_entries {
            out.truncate(ctx.limits.max_traversal_entries);
            out.push(VirtualPath::from("/[find results truncated]"));
        }
        Ok(CommandOutput::stdout(join_lines(
            out.into_iter().map(|p| p.to_string()),
        )))
    }
}

fn walk(
    ctx: &CommandContext,
    root: &VirtualPath,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<VirtualPath>,
) -> ShellResult<()> {
    out.push(root.clone());
    if depth >= max_depth || out.len() >= ctx.limits.max_traversal_entries {
        return Ok(());
    }
    if ctx.fs.metadata(root)?.is_dir {
        for entry in ctx.fs.list_dir(root)? {
            walk(ctx, &entry.path, depth + 1, max_depth, out)?;
            if out.len() >= ctx.limits.max_traversal_entries {
                break;
            }
        }
    }
    Ok(())
}

fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        return name.starts_with(prefix) && name.ends_with(suffix);
    }
    pattern == name
}

struct Tree;
impl BuiltinCommand for Tree {
    fn name(&self) -> &'static str {
        "tree"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let mut max_depth = ctx.limits.max_recursion_depth.min(4);
        let mut root_arg = None;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "-L" {
                i += 1;
                max_depth = args
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(max_depth);
            } else if !args[i].starts_with('-') {
                root_arg = Some(&args[i]);
            }
            i += 1;
        }
        let root = path(ctx, root_arg)?;
        let mut lines = vec![root.to_string()];
        tree_walk(ctx, &root, 0, max_depth, &mut lines)?;
        Ok(CommandOutput::stdout(join_lines(lines)))
    }
}

fn tree_walk(
    ctx: &CommandContext,
    root: &VirtualPath,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) -> ShellResult<()> {
    if depth >= max_depth || lines.len() >= ctx.limits.max_traversal_entries {
        return Ok(());
    }
    for entry in ctx.fs.list_dir(root).unwrap_or_default() {
        lines.push(format!("{}{}", "  ".repeat(depth + 1), entry.name));
        if entry.metadata.is_dir {
            tree_walk(ctx, &entry.path, depth + 1, max_depth, lines)?;
        }
        if lines.len() >= ctx.limits.max_traversal_entries {
            lines.push("[tree output truncated]".into());
            break;
        }
    }
    Ok(())
}

struct Sed;
impl BuiltinCommand for Sed {
    fn name(&self) -> &'static str {
        "sed"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let mut args = args.iter();
        let mut quiet = false;
        let first = args
            .next()
            .ok_or_else(|| ShellError::InvalidInput("sed: missing script".into()))?;
        let script = if first == "-n" {
            quiet = true;
            args.next()
                .ok_or_else(|| ShellError::InvalidInput("sed: missing script".into()))?
        } else {
            first
        };
        let file = args
            .next()
            .ok_or_else(|| ShellError::InvalidInput("sed: missing file".into()))?;
        let text = read_text(ctx, &path(ctx, Some(file))?)?;
        if !quiet {
            return Ok(CommandOutput::stdout(text));
        }
        let range = script.strip_suffix('p').ok_or_else(|| {
            ShellError::Unsupported("sed: only print ranges are supported".into())
        })?;
        let (start, end) = if let Some((a, b)) = range.split_once(',') {
            (
                a.parse::<usize>().unwrap_or(1),
                b.parse::<usize>().unwrap_or(usize::MAX),
            )
        } else {
            let line = range.parse::<usize>().unwrap_or(1);
            (line, line)
        };
        Ok(CommandOutput::stdout(join_lines(
            text.lines()
                .enumerate()
                .filter(|(ix, _)| ix + 1 >= start && ix + 1 <= end)
                .map(|(_, line)| line.to_string()),
        )))
    }
}

struct Test;
impl BuiltinCommand for Test {
    fn name(&self) -> &'static str {
        "test"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        let (flag, target) = match args {
            [flag, target] => (flag.as_str(), target),
            [target] => ("-e", target),
            _ => {
                return Err(ShellError::InvalidInput(
                    "test: unsupported expression".into(),
                ));
            }
        };
        let path = path(ctx, Some(target))?;
        let result = ctx.fs.metadata(&path).map(|meta| match flag {
            "-e" => true,
            "-f" => meta.is_file,
            "-d" => meta.is_dir,
            _ => false,
        });
        Ok(CommandOutput::code(if result.unwrap_or(false) {
            0
        } else {
            1
        }))
    }
}

struct Mkdir;
impl BuiltinCommand for Mkdir {
    fn name(&self) -> &'static str {
        "mkdir"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        ensure_writes_allowed(&ctx.policy)?;
        let dirs = args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .collect::<Vec<_>>();
        if dirs.is_empty() {
            return Err(ShellError::InvalidInput("mkdir: missing operand".into()));
        }
        for dir in dirs {
            ctx.fs.create_dir_all(&path(ctx, Some(dir))?)?;
        }
        Ok(CommandOutput::default())
    }
}

struct Touch;
impl BuiltinCommand for Touch {
    fn name(&self) -> &'static str {
        "touch"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        ensure_writes_allowed(&ctx.policy)?;
        if args.is_empty() {
            return Err(ShellError::InvalidInput("touch: missing file".into()));
        }
        for file in args {
            let path = path(ctx, Some(file))?;
            let data = ctx.fs.read_file(&path).unwrap_or_default();
            ctx.fs.write_file(&path, &data)?;
        }
        Ok(CommandOutput::default())
    }
}

struct Rm;
impl BuiltinCommand for Rm {
    fn name(&self) -> &'static str {
        "rm"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        ensure_writes_allowed(&ctx.policy)?;
        let mut recursive = false;
        let mut files = Vec::new();
        for arg in args {
            if arg.starts_with('-') {
                recursive |= arg.contains('r') || arg.contains('R');
            } else {
                files.push(arg);
            }
        }
        if recursive && !ctx.policy.allow_recursive_delete {
            return Err(ShellError::AccessDenied(
                "recursive delete is disabled by shell policy".into(),
            ));
        }
        for file in files {
            let path = path(ctx, Some(file))?;
            if path.as_str() == "/" || path.as_str() == "/workspace" {
                return Err(ShellError::AccessDenied(format!(
                    "rm: refusing to remove {path}"
                )));
            }
            if recursive {
                ctx.fs.remove_dir_all(&path)?;
            } else {
                ctx.fs.remove_file(&path)?;
            }
        }
        Ok(CommandOutput::default())
    }
}

struct Cp;
impl BuiltinCommand for Cp {
    fn name(&self) -> &'static str {
        "cp"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        ensure_writes_allowed(&ctx.policy)?;
        let [from, to] = args else {
            return Err(ShellError::InvalidInput(
                "cp: expected source and dest".into(),
            ));
        };
        let data = ctx.fs.read_file(&path(ctx, Some(from))?)?;
        ctx.fs.write_file(&path(ctx, Some(to))?, &data)?;
        Ok(CommandOutput::default())
    }
}

struct Mv;
impl BuiltinCommand for Mv {
    fn name(&self) -> &'static str {
        "mv"
    }

    fn run(&self, ctx: &mut CommandContext, args: &[String]) -> ShellResult<CommandOutput> {
        ensure_writes_allowed(&ctx.policy)?;
        let [from, to] = args else {
            return Err(ShellError::InvalidInput(
                "mv: expected source and dest".into(),
            ));
        };
        ctx.fs
            .rename(&path(ctx, Some(from))?, &path(ctx, Some(to))?)?;
        Ok(CommandOutput::default())
    }
}

#[allow(dead_code)]
fn _sort_entries(entries: &mut [VirtualDirEntry]) {
    entries.sort_by(|a, b| a.name.cmp(&b.name));
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use tempfile::tempdir;

    use super::*;
    use crate::fs::{InMemoryFs, OverlayFs, WorkspaceFs};
    use crate::{ExecRequest, Shell};

    fn shell(root: &std::path::Path, allow_writes: bool) -> Shell {
        let mut policy = ShellPolicy::default();
        policy.allow_writes = allow_writes;
        policy.allow_recursive_delete = false;
        Shell::new(
            Arc::new(OverlayFs::new(root, policy.max_file_read_bytes)),
            CommandRegistry::with_defaults(),
            ExecutionLimits {
                max_output_bytes: 8 * 1024,
                max_file_read_bytes: 8 * 1024,
                max_traversal_entries: 100,
                max_grep_matches: 20,
                ..ExecutionLimits::default()
            },
            policy,
            VirtualPath::workspace(),
        )
    }

    fn exec(shell: &mut Shell, command: &str) -> crate::ExecResult {
        shell.exec(ExecRequest {
            command: command.into(),
            cwd: None,
            env: vec![],
            max_output_bytes: 8 * 1024,
            timeout_ms: 30_000,
        })
    }

    #[test]
    fn memory_fs_writes_and_reads() {
        let fs = InMemoryFs::new();
        let path = VirtualPath::from("/tmp/a.txt");
        fs.write_file(&path, b"hello").unwrap();
        assert_eq!(fs.read_file(&path).unwrap(), b"hello");
    }

    #[test]
    fn workspace_read_and_escape_blocks() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        let fs = WorkspaceFs::new(dir.path(), true, 1024);
        assert_eq!(
            fs.read_file(&VirtualPath::from("/workspace/main.rs"))
                .unwrap(),
            b"fn main() {}\n"
        );
        assert!(fs.read_file(&VirtualPath::from("/etc/passwd")).is_err());
        assert!(
            fs.read_file(&VirtualPath::from("/Users/me/.ssh/id_rsa"))
                .is_err()
        );
    }

    #[test]
    fn readonly_workspace_rejects_write() {
        let dir = tempdir().unwrap();
        let fs = WorkspaceFs::new(dir.path(), true, 1024);
        assert!(
            fs.write_file(&VirtualPath::from("/workspace/new.txt"), b"x")
                .is_err()
        );
    }

    #[test]
    fn overlay_write_read_does_not_touch_workspace() {
        let dir = tempdir().unwrap();
        let fs = OverlayFs::new(dir.path(), 1024);
        let path = VirtualPath::from("/workspace/generated.txt");
        fs.write_file(&path, b"virtual").unwrap();
        assert_eq!(fs.read_file(&path).unwrap(), b"virtual");
        assert!(!dir.path().join("generated.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_blocked() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(
            outside.path().join("secret.txt"),
            dir.path().join("link.txt"),
        )
        .unwrap();
        let fs = WorkspaceFs::new(dir.path(), true, 1024);
        assert!(
            fs.read_file(&VirtualPath::from("/workspace/link.txt"))
                .is_err()
        );
    }

    #[test]
    fn builtins_cover_common_read_commands() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "one\ntwo\nthree\n").unwrap();
        fs::write(dir.path().join("src/lib.rs"), "two\n").unwrap();
        let mut sh = shell(dir.path(), false);

        assert_eq!(exec(&mut sh, "pwd").stdout, "/workspace\n");
        assert!(exec(&mut sh, "ls src").stdout.contains("main.rs"));
        assert_eq!(exec(&mut sh, "cat src/lib.rs").stdout, "two\n");
        assert_eq!(exec(&mut sh, "head -n 1 src/main.rs").stdout, "one\n");
        assert_eq!(exec(&mut sh, "tail -n 1 src/main.rs").stdout, "three\n");
        assert!(exec(&mut sh, "wc -l src/main.rs").stdout.starts_with("3 "));
        assert!(
            exec(&mut sh, "grep -R -n two src")
                .stdout
                .contains("src/main.rs:2:two")
        );
        assert!(
            exec(&mut sh, "find src -name '*.rs' -maxdepth 2")
                .stdout
                .contains("lib.rs")
        );
        assert!(exec(&mut sh, "tree -L 2 src").stdout.contains("main.rs"));
        assert_eq!(
            exec(&mut sh, "sed -n '2,3p' src/main.rs").stdout,
            "two\nthree\n"
        );
        assert_eq!(exec(&mut sh, "test -f src/main.rs").exit_code, 0);
        assert_eq!(exec(&mut sh, "cargo test").exit_code, 127);
    }

    #[test]
    fn write_commands_are_policy_gated_and_overlay_only() {
        let dir = tempdir().unwrap();
        let mut read_only = shell(dir.path(), false);
        assert_eq!(exec(&mut read_only, "touch new.txt").exit_code, 1);

        let mut writable = shell(dir.path(), true);
        assert_eq!(exec(&mut writable, "mkdir -p tmp").exit_code, 0);
        assert_eq!(exec(&mut writable, "touch tmp/file").exit_code, 0);
        assert_eq!(exec(&mut writable, "test -f tmp/file").exit_code, 0);
        assert!(!dir.path().join("tmp/file").exists());
        assert_eq!(exec(&mut writable, "rm -rf /workspace").exit_code, 1);
        assert_eq!(exec(&mut writable, "rm -rf tmp").exit_code, 1);
    }

    #[test]
    fn output_truncation_works() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("big.txt"), "x".repeat(1_000)).unwrap();
        let mut sh = shell(dir.path(), false);
        let result = sh.exec(ExecRequest {
            command: "cat big.txt".into(),
            cwd: None,
            env: vec![],
            max_output_bytes: 64,
            timeout_ms: 30_000,
        });
        assert!(result.truncated);
    }

    #[test]
    fn agent_shell_source_has_no_process_or_network_execution() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![root];
        let blocked = [
            concat!("std::process::", "Command"),
            concat!("tokio::process::", "Command"),
            concat!("Command", "::new"),
            concat!("sh", " -c"),
            concat!("req", "west"),
            concat!("u", "req"),
        ];
        while let Some(path) = stack.pop() {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|v| v.to_str()) != Some("rs") {
                    continue;
                }
                let source = fs::read_to_string(&path).unwrap();
                for needle in blocked {
                    assert!(
                        !source.contains(needle),
                        "{} contains blocked token {}",
                        path.display(),
                        needle
                    );
                }
            }
        }
    }
}
