use core::panic;
use std::println;
use std::{env, process::Command};
use std::fs::read_link;
use std::io::{self, Write};

use serde_json;

use hyprland::data::Client;
use hyprland::prelude::HyprDataActiveOptional;

use detect_desktop_environment::DesktopEnvironment;


use crate::{ActiveWindow, WindowPosition};

fn try_hyprland() -> Option<ActiveWindow> {
    env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;

    let info = Client::get_active().ok()??;
    let process_id = info.pid.try_into().ok()?;
    let process_path = read_link(format!("/proc/{}/exe", info.pid)).unwrap_or_default();

    Some(ActiveWindow {
        title: info.title,
        app_name: info.class,
        window_id: info.address.to_string(),
        process_id,
        process_path,
        position: WindowPosition {
            x: f64::from(info.at.0),
            y: f64::from(info.at.1),
            width: f64::from(info.size.0),
            height: f64::from(info.size.1),
        },
    })
}

fn try_kwin() -> Option<ActiveWindow> {
    // Use kdotool library to get active window info
    let info = kdotool::get_active_window_info().ok()?;

    let process_path = read_link(format!("/proc/{}/exe", info.pid)).unwrap_or_default();

    Some(ActiveWindow {
        title: info.title,
        app_name: info.class_name,
        window_id: info.id,
        process_id: info.pid as u64,
        process_path,
        position: WindowPosition {
            x: info.x,
            y: info.y,
            width: info.width,
            height: info.height,
        },
    })
}

fn try_gnome() -> Option<ActiveWindow> {
    // Install extension if needed and use Window Calls Extended
    let uuid = "window-calls@domandoman.xyz";

    if !gnome_extension(uuid) {
        gnome_extension_install(uuid);
    }

    let window_list: serde_json::Value = serde_json::from_slice(
        &Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply=literal",
                "--dest=org.gnome.Shell",
                "/org/gnome/Shell/Extensions/Windows",
                "org.gnome.Shell.Extensions.Windows.List",
            ])
            .output()
            .unwrap()
            .stdout,
    ).unwrap();


    let focused_window_id = format!(
        "uint32:{}",
        window_list
            .as_array()
            .unwrap()
            .iter()
            .find(|window| window["focus"] == true)
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    );

    let info: serde_json::Value = serde_json::from_slice(
        &Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.gnome.Shell",
            "/org/gnome/Shell/Extensions/Windows",
            "org.gnome.Shell.Extensions.Windows.Details",
            &focused_window_id
        ])
        .output()
        .unwrap()
        .stdout,
    ).unwrap();

    let process_path = read_link(format!("/proc/{}/exe", info["pid"].as_u64().unwrap())).unwrap_or_default();

     Some(ActiveWindow {
        title: info["title"].to_string(),
        app_name: info["wm_class"].to_string(),
        window_id: info["id"].to_string(),
        process_id: info["pid"].as_u64().unwrap(),
        process_path,
        position: WindowPosition {
            x: info["x"].as_f64().unwrap(),
            y: info["y"].as_f64().unwrap(),
            width: info["width"].as_f64().unwrap(),
            height: info["height"].as_f64().unwrap(),
        },
    })
}

fn gnome_extension(uuid: &str) -> bool {
    let is_installed = Command::new("gnome-extensions")
        .args(["list", "--enabled"])
        .output()
        .unwrap()
        .stdout
        .split(|&b| b == b'\n')
        .any(|line| line == uuid.as_bytes());
    
    is_installed
}

fn gnome_extension_install(uuid: &str) {

    println!("Window Call extension not found. Installing now...");

    Command::new("curl")
    .args([
        "-L",
        "-O",
        "https://extensions.gnome.org/extension-data/window-callsdomandoman.xyz.v21.shell-extension.zip",
    ])
    .status()
    .unwrap();
    
    Command::new("gnome-extensions")
    .args([
        "install",
        "window-callsdomandoman.xyz.v21.shell-extension.zip",
    ])
    .status()
    .unwrap();

    Command::new("gdbus")
    .args([
        "call",
        "--session",
        "--dest", "org.gnome.Shell",
        "--object-path", "/org/gnome/Shell",
        "--method", "org.gnome.Shell.Extensions.EnableExtension",
        uuid,
    ])
    .output()
    .unwrap();

    println!("Installed Window Call extension");

    print!("GNOME Shell needs to be restarted. Restart now? [y/N] ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    if input.trim().eq_ignore_ascii_case("y") {
        Command::new("gnome-session-quit")
            .args(["--logout", "--no-prompt"])
            .status()
            .unwrap();
    }
    
}

pub fn get_active_window_wayland() -> Option<ActiveWindow> {
    match DesktopEnvironment::detect() {
        Some(DesktopEnvironment::Kde) => try_kwin(),
        Some(DesktopEnvironment::Hyprland) => try_hyprland(),
        Some(DesktopEnvironment::Gnome) => try_gnome(),
        Some(de) => panic!(),
        None => panic!(),
    }
}
