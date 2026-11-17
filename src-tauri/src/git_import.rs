use crate::{
    library::{checked_path, create_directory, create_file},
    project::{display_path, Result},
};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn head(source: &Path) -> Result<String> {
    if run(
        source,
        &["rev-parse", "--is-inside-work-tree", "--show-prefix"],
    )?
    .trim_end()
        != "true"
    {
        return Err("请选择本地 Git 仓库的根目录。".into());
    }
    revision(source)
}

pub fn copy_tree(source: &Path, id: &str, workspace: &Path) -> Result<()> {
    let tree = run(source, &["ls-tree", "-rlz", "--full-tree", id])?;
    let entries = tree
        .split_terminator('\0')
        .map(|record| {
            let (header, name) = record.split_once('\t').ok_or("无法解析 Git 文件列表。")?;
            let mut fields = header.split_whitespace();
            if !matches!(fields.next(), Some("100644" | "100755")) || fields.next() != Some("blob")
            {
                return Err(format!("Git 提交包含不支持的文件类型：{name}"));
            }
            let id = fields.next().ok_or("Git 文件缺少对象 ID。")?;
            let size = fields
                .next()
                .and_then(|size| size.parse::<u64>().ok())
                .ok_or("Git 文件缺少有效的大小。")?;
            Ok((id, size, workspace.join(checked_path(name)?)))
        })
        .collect::<Result<Vec<_>>>()?;
    copy_blobs(source, entries)
}

fn copy_blobs(source: &Path, entries: Vec<(&str, u64, PathBuf)>) -> Result<()> {
    let mut child = git(source)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法启动 Git：{error}"))?;
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let copied = (|| -> Result<()> {
        let mut header = String::new();
        for (id, size, path) in entries {
            writeln!(input, "{id}").map_err(|error| format!("无法请求 Git 文件：{error}"))?;
            header.clear();
            output
                .read_line(&mut header)
                .map_err(|error| format!("无法读取 Git 文件头：{error}"))?;
            if header.trim_end() != format!("{id} blob {size}") {
                return Err(format!("Git 缺少文件对象：{}", path.display()));
            }
            create_directory(path.parent().ok_or("Git 文件路径无效。")?)?;
            create_file(&path, &mut (&mut output).take(size), size)?;
            let mut separator = [0u8];
            output
                .read_exact(&mut separator)
                .map_err(|error| format!("Git 文件不完整：{error}"))?;
            if separator != [b'\n'] {
                return Err("Git 文件响应无效。".into());
            }
        }
        Ok(())
    })();
    drop(input);
    drop(output);
    if copied.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|error| format!("无法等待 Git 退出：{error}"))?;
    copied?;
    if !status.success() {
        return Err("Git 未能读取提交中的全部文件。".into());
    }
    Ok(())
}

pub fn baseline(workspace: &Path) -> Result<String> {
    run(
        workspace,
        &["init", "-q", "--template=", "--initial-branch=studio"],
    )?;
    let info = workspace.join(".git").join("info");
    fs::create_dir_all(&info)
        .and_then(|_| {
            fs::write(
                info.join("attributes"),
                "* -text -crlf -ident -filter -working-tree-encoding\n",
            )
        })
        .map_err(|error| format!("无法设置 Studio 版本库：{error}"))?;
    run(workspace, &["add", "-f", "--all", "--", "."])?;
    run(
        workspace,
        &[
            "-c",
            "user.name=Novel Studio",
            "-c",
            "user.email=studio@localhost",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "Studio import baseline",
        ],
    )?;
    revision(workspace)
}

fn revision(directory: &Path) -> Result<String> {
    let result = run(directory, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let id = result.trim();
    if !matches!(id.len(), 40 | 64) || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Git 返回了无效的提交标识。".into());
    }
    Ok(id.to_owned())
}

fn git(directory: &Path) -> Command {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_ALLOW_PROTOCOL", "")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .args(["-c", "core.longpaths=true", "--no-optional-locks", "-C"])
        .arg(display_path(directory));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn run(directory: &Path, args: &[&str]) -> Result<String> {
    let output = git(directory)
        .args(args)
        .output()
        .map_err(|error| format!("无法启动 Git，请确认 Git 已安装且可从 PATH 运行：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git 操作失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "Git 提交包含无法识别的文件名。".into())
}
