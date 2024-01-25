// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{
  helpers::{
    app_paths::{app_dir, tauri_dir},
    command_env,
    config::{get as get_config, reload as reload_config, AppUrl, BeforeDevCommand, WebviewUrl},
    resolve_merge_config,
  },
  interface::{AppInterface, DevProcess, ExitReason, Interface},
  CommandExt, Result,
};

use anyhow::{bail, Context};
use clap::{ArgAction, Parser};
use log::{error, info, warn};
use shared_child::SharedChild;
use tauri_utils::platform::Target;

use std::{
  env::set_current_dir,
  net::{IpAddr, Ipv4Addr},
  process::{exit, Command, Stdio},
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
  },
};

static BEFORE_DEV: OnceLock<Mutex<Arc<SharedChild>>> = OnceLock::new();
static KILL_BEFORE_DEV_FLAG: OnceLock<AtomicBool> = OnceLock::new();

#[cfg(unix)]
const KILL_CHILDREN_SCRIPT: &[u8] = include_bytes!("../scripts/kill-children.sh");

pub const TAURI_CLI_BUILTIN_WATCHER_IGNORE_FILE: &[u8] =
  include_bytes!("../tauri-dev-watcher.gitignore");

#[derive(Debug, Clone, Parser)]
#[clap(
  about = "Run your app in development mode",
  long_about = "Run your app in development mode with hot-reloading for the Rust code. It makes use of the `build.devPath` property from your `tauri.conf.json` file. It also runs your `build.beforeDevCommand` which usually starts your frontend devServer.",
  trailing_var_arg(true)
)]
pub struct Options {
  /// Binary to use to run the application
  #[clap(short, long)]
  pub runner: Option<String>,
  /// Target triple to build against
  #[clap(short, long)]
  pub target: Option<String>,
  /// List of cargo features to activate
  #[clap(short, long, action = ArgAction::Append, num_args(0..))]
  pub features: Option<Vec<String>>,
  /// Exit on panic
  #[clap(short, long)]
  pub exit_on_panic: bool,
  /// JSON string or path to JSON file to merge with tauri.conf.json
  #[clap(short, long)]
  pub config: Option<String>,
  /// Run the code in release mode
  #[clap(long = "release")]
  pub release_mode: bool,
  /// Command line arguments passed to the runner.
  /// Use `--` to explicitly mark the start of the arguments. Arguments after a second `--` are passed to the application
  /// e.g. `tauri dev -- [runnerArgs] -- [appArgs]`.
  pub args: Vec<String>,
  /// Skip waiting for the frontend dev server to start before building the tauri application.
  #[clap(long, env = "TAURI_CLI_NO_DEV_SERVER_WAIT")]
  pub no_dev_server_wait: bool,
  /// Disable the file watcher.
  #[clap(long)]
  pub no_watch: bool,
  /// Force prompting for an IP to use to connect to the dev server on mobile.
  #[clap(long)]
  pub force_ip_prompt: bool,

  /// Disable the built-in dev server for static files.
  #[clap(long)]
  pub no_dev_server: bool,
  /// Specify port for the built-in dev server for static files. Defaults to 1430.
  #[clap(long, env = "TAURI_CLI_PORT")]
  pub port: Option<u16>,
}

pub fn command(options: Options) -> Result<()> {
  let r = command_internal(options);
  if r.is_err() {
    kill_before_dev_process();
  }
  r
}

fn command_internal(mut options: Options) -> Result<()> {
  let target = options
    .target
    .as_deref()
    .map(Target::from_triple)
    .unwrap_or_else(Target::current);
  let mut interface = setup(target, &mut options, false)?;
  let exit_on_panic = options.exit_on_panic;
  let no_watch = options.no_watch;
  interface.dev(options.into(), move |status, reason| {
    on_app_exit(status, reason, exit_on_panic, no_watch)
  })
}

pub fn local_ip_address(force: bool) -> &'static IpAddr {
  static LOCAL_IP: OnceLock<IpAddr> = OnceLock::new();
  LOCAL_IP.get_or_init(|| {
    let prompt_for_ip = || {
      let addresses: Vec<IpAddr> = local_ip_address::list_afinet_netifas()
        .expect("failed to list networks")
        .into_iter()
        .map(|(_, ipaddr)| ipaddr)
        .filter(|ipaddr| match ipaddr {
          IpAddr::V4(i) => i != &Ipv4Addr::LOCALHOST,
          _ => false,
        })
        .collect();
      match addresses.len() {
        0 => panic!("No external IP detected."),
        1 => {
          let ipaddr = addresses.first().unwrap();
          *ipaddr
        }
        _ => {
          let selected = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(
              "Failed to detect external IP, What IP should we use to access your development server?",
            )
            .items(&addresses)
            .default(0)
            .interact()
            .expect("failed to select external IP");
          *addresses.get(selected).unwrap()
        }
      }
    };

    let ip = if force {
      prompt_for_ip()
    } else {
      local_ip_address::local_ip().unwrap_or_else(|_| prompt_for_ip())
    };
    log::info!("Using {ip} to access the development server.");
    ip
  })
}

pub fn setup(target: Target, options: &mut Options, mobile: bool) -> Result<AppInterface> {
  let (merge_config, _merge_config_path) = resolve_merge_config(&options.config)?;
  options.config = merge_config;

  let config = get_config(target, options.config.as_deref())?;

  let tauri_path = tauri_dir();
  set_current_dir(tauri_path).with_context(|| "failed to change current working directory")?;

  let interface = AppInterface::new(
    config.lock().unwrap().as_ref().unwrap(),
    options.target.clone(),
  )?;

  let mut dev_path = config
    .lock()
    .unwrap()
    .as_ref()
    .unwrap()
    .build
    .dev_path
    .clone();

  if let Some(before_dev) = config
    .lock()
    .unwrap()
    .as_ref()
    .unwrap()
    .build
    .before_dev_command
    .clone()
  {
    let (script, script_cwd, wait) = match before_dev {
      BeforeDevCommand::Script(s) if s.is_empty() => (None, None, false),
      BeforeDevCommand::Script(s) => (Some(s), None, false),
      BeforeDevCommand::ScriptWithOptions { script, cwd, wait } => {
        (Some(script), cwd.map(Into::into), wait)
      }
    };
    let cwd = script_cwd.unwrap_or_else(|| app_dir().clone());
    if let Some(mut before_dev) = script {
      if before_dev.contains("$HOST") {
        if mobile {
          let local_ip_address = local_ip_address(options.force_ip_prompt).to_string();
          before_dev = before_dev.replace("$HOST", &local_ip_address);
          if let AppUrl::Url(WebviewUrl::External(url)) = &mut dev_path {
            url.set_host(Some(&local_ip_address))?;
          }
        } else {
          before_dev = before_dev.replace(
            "$HOST",
            if let AppUrl::Url(WebviewUrl::External(url)) = &dev_path {
              url.host_str().unwrap_or("0.0.0.0")
            } else {
              "0.0.0.0"
            },
          );
        }
      }
      info!(action = "Running"; "BeforeDevCommand (`{}`)", before_dev);
      let mut env = command_env(true);
      env.extend(interface.env());

      #[cfg(windows)]
      let mut command = {
        let mut command = Command::new("cmd");
        command
          .arg("/S")
          .arg("/C")
          .arg(&before_dev)
          .current_dir(cwd)
          .envs(env);
        command
      };
      #[cfg(not(windows))]
      let mut command = {
        let mut command = Command::new("sh");
        command
          .arg("-c")
          .arg(&before_dev)
          .current_dir(cwd)
          .envs(env);
        command
      };

      if wait {
        let status = command.piped().with_context(|| {
          format!(
            "failed to run `{}` with `{}`",
            before_dev,
            if cfg!(windows) { "cmd /S /C" } else { "sh -c" }
          )
        })?;
        if !status.success() {
          bail!(
            "beforeDevCommand `{}` failed with exit code {}",
            before_dev,
            status.code().unwrap_or_default()
          );
        }
      } else {
        command.stdin(Stdio::piped());
        command.stdout(os_pipe::dup_stdout()?);
        command.stderr(os_pipe::dup_stderr()?);

        let child = SharedChild::spawn(&mut command)
          .unwrap_or_else(|_| panic!("failed to run `{before_dev}`"));
        let child = Arc::new(child);
        let child_ = child.clone();

        std::thread::spawn(move || {
          let status = child_
            .wait()
            .expect("failed to wait on \"beforeDevCommand\"");
          if !(status.success() || KILL_BEFORE_DEV_FLAG.get().unwrap().load(Ordering::Relaxed)) {
            error!("The \"beforeDevCommand\" terminated with a non-zero status code.");
            exit(status.code().unwrap_or(1));
          }
        });

        BEFORE_DEV.set(Mutex::new(child)).unwrap();
        KILL_BEFORE_DEV_FLAG.set(AtomicBool::default()).unwrap();

        let _ = ctrlc::set_handler(move || {
          kill_before_dev_process();
          exit(130);
        });
      }
    }
  }

  if options.runner.is_none() {
    options.runner = config
      .lock()
      .unwrap()
      .as_ref()
      .unwrap()
      .build
      .runner
      .clone();
  }

  let mut cargo_features = config
    .lock()
    .unwrap()
    .as_ref()
    .unwrap()
    .build
    .features
    .clone()
    .unwrap_or_default();
  if let Some(features) = &options.features {
    cargo_features.extend(features.clone());
  }

  let mut dev_path = config
    .lock()
    .unwrap()
    .as_ref()
    .unwrap()
    .build
    .dev_path
    .clone();
  if !options.no_dev_server {
    if let AppUrl::Url(WebviewUrl::App(path)) = &dev_path {
      use crate::helpers::web_dev_server::start_dev_server;
      if path.exists() {
        let path = path.canonicalize()?;
        let ip = if mobile {
          *local_ip_address(options.force_ip_prompt)
        } else {
          Ipv4Addr::new(127, 0, 0, 1).into()
        };
        let server_url = start_dev_server(path, ip, options.port)?;
        let server_url = format!("http://{server_url}");
        dev_path = AppUrl::Url(WebviewUrl::External(server_url.parse().unwrap()));

        if let Some(c) = &options.config {
          let mut c: tauri_utils::config::Config = serde_json::from_str(c)?;
          c.build.dev_path = dev_path.clone();
          options.config = Some(serde_json::to_string(&c).unwrap());
        } else {
          options.config = Some(format!(r#"{{ "build": {{ "devPath": "{server_url}" }} }}"#))
        }

        reload_config(options.config.as_deref())?;
      }
    }
  }

  if !options.no_dev_server_wait {
    if let AppUrl::Url(WebviewUrl::External(dev_server_url)) = dev_path {
      let host = dev_server_url
        .host()
        .unwrap_or_else(|| panic!("No host name in the URL"));
      let port = dev_server_url
        .port_or_known_default()
        .unwrap_or_else(|| panic!("No port number in the URL"));
      let addrs;
      let addr;
      let addrs = match host {
        url::Host::Domain(domain) => {
          use std::net::ToSocketAddrs;
          addrs = (domain, port).to_socket_addrs()?;
          addrs.as_slice()
        }
        url::Host::Ipv4(ip) => {
          addr = (ip, port).into();
          std::slice::from_ref(&addr)
        }
        url::Host::Ipv6(ip) => {
          addr = (ip, port).into();
          std::slice::from_ref(&addr)
        }
      };
      let mut i = 0;
      let sleep_interval = std::time::Duration::from_secs(2);
      let timeout_duration = std::time::Duration::from_secs(1);
      let max_attempts = 90;
      'waiting: loop {
        for addr in addrs.iter() {
          if std::net::TcpStream::connect_timeout(addr, timeout_duration).is_ok() {
            break 'waiting;
          }
        }

        if i % 3 == 1 {
          warn!(
            "Waiting for your frontend dev server to start on {}...",
            dev_server_url
          );
        }
        i += 1;
        if i == max_attempts {
          error!(
            "Could not connect to `{}` after {}s. Please make sure that is the URL to your dev server.",
            dev_server_url, i * sleep_interval.as_secs()
          );
          exit(1);
        }
        std::thread::sleep(sleep_interval);
      }
    }
  }

  Ok(interface)
}

pub fn wait_dev_process<
  C: DevProcess + Send + 'static,
  F: Fn(Option<i32>, ExitReason) + Send + Sync + 'static,
>(
  child: C,
  on_exit: F,
) {
  std::thread::spawn(move || {
    let code = child
      .wait()
      .ok()
      .and_then(|status| status.code())
      .or(Some(1));
    on_exit(
      code,
      if child.manually_killed_process() {
        ExitReason::TriggeredKill
      } else {
        ExitReason::NormalExit
      },
    );
  });
}

pub fn on_app_exit(code: Option<i32>, reason: ExitReason, exit_on_panic: bool, no_watch: bool) {
  if no_watch
    || (!matches!(reason, ExitReason::TriggeredKill)
      && (exit_on_panic || matches!(reason, ExitReason::NormalExit)))
  {
    kill_before_dev_process();
    exit(code.unwrap_or(0));
  }
}

pub fn kill_before_dev_process() {
  if let Some(child) = BEFORE_DEV.get() {
    let child = child.lock().unwrap();
    let kill_before_dev_flag = KILL_BEFORE_DEV_FLAG.get().unwrap();
    if kill_before_dev_flag.load(Ordering::Relaxed) {
      return;
    }
    kill_before_dev_flag.store(true, Ordering::Relaxed);
    #[cfg(windows)]
    {
      let powershell_path = std::env::var("SYSTEMROOT").map_or_else(
        |_| "powershell.exe".to_string(),
        |p| format!("{p}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
      );
      let _ = Command::new(powershell_path)
      .arg("-NoProfile")
      .arg("-Command")
      .arg(format!("function Kill-Tree {{ Param([int]$ppid); Get-CimInstance Win32_Process | Where-Object {{ $_.ParentProcessId -eq $ppid }} | ForEach-Object {{ Kill-Tree $_.ProcessId }}; Stop-Process -Id $ppid -ErrorAction SilentlyContinue }}; Kill-Tree {}", child.id()))
      .status();
    }
    #[cfg(unix)]
    {
      use std::io::Write;
      let mut kill_children_script_path = std::env::temp_dir();
      kill_children_script_path.push("kill-children.sh");

      if !kill_children_script_path.exists() {
        if let Ok(mut file) = std::fs::File::create(&kill_children_script_path) {
          use std::os::unix::fs::PermissionsExt;
          let _ = file.write_all(KILL_CHILDREN_SCRIPT);
          let mut permissions = file.metadata().unwrap().permissions();
          permissions.set_mode(0o770);
          let _ = file.set_permissions(permissions);
        }
      }
      let _ = Command::new(&kill_children_script_path)
        .arg(child.id().to_string())
        .output();
    }
    let _ = child.kill();
  }
}
