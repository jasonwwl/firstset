#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod config;
mod i18n;
mod runner;
mod scripts;

use std::{env, fs, path::PathBuf};

#[cfg(target_os = "windows")]
use std::sync::Arc;

use app::{SetupApp, configure_style};
use config::AppConfig;
use eframe::egui;

fn main() {
    if let Err(error) = run() {
        let message = format!("GUI startup failed: {error:#}");
        let path = configuration_path().with_file_name("startup-error.log");
        let _ = fs::write(path, message);
        std::process::exit(1);
    }
}

fn run() -> eframe::Result {
    let config_path = configuration_path();
    let (config, startup_message) = match AppConfig::load_or_create(&config_path) {
        Ok(config) => (config, None),
        Err(error) => (
            AppConfig::default(),
            Some(format!("ERROR: failed to load configuration: {error:#}")),
        ),
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FirstSet")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1080.0, 640.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_options(),
        ..Default::default()
    };

    eframe::run_native(
        "FirstSet",
        options,
        Box::new(move |creation_context| {
            configure_style(&creation_context.egui_ctx);
            Ok(Box::new(SetupApp::new(
                config,
                config_path,
                startup_message,
            )))
        }),
    )
}

fn wgpu_options() -> eframe::WgpuConfiguration {
    let configuration = eframe::WgpuConfiguration::default()
        .with_surface_config(eframe::SurfaceConfig::LOW_LATENCY);

    #[cfg(target_os = "windows")]
    {
        let mut configuration = configuration;
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut configuration.wgpu_setup {
            // RDS sessions expose Microsoft Remote Display Adapter, which often has no
            // usable OpenGL 2.0 context. Direct3D 12 works with the physical adapter and
            // can fall back to Windows' WARP software renderer on headless cloud servers.
            setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
            setup.power_preference = eframe::wgpu::PowerPreference::LowPower;
            setup.native_adapter_selector = Some(Arc::new(select_windows_adapter));
        }
        configuration
    }

    #[cfg(not(target_os = "windows"))]
    {
        configuration
    }
}

#[cfg(target_os = "windows")]
fn select_windows_adapter(
    adapters: &[eframe::wgpu::Adapter],
    surface: Option<&eframe::wgpu::Surface<'_>>,
) -> Result<eframe::wgpu::Adapter, String> {
    let selected = adapters
        .iter()
        .filter(|adapter| surface.is_none_or(|surface| adapter.is_surface_supported(surface)))
        .min_by_key(|adapter| match adapter.get_info().device_type {
            eframe::wgpu::DeviceType::IntegratedGpu => 0,
            eframe::wgpu::DeviceType::DiscreteGpu => 1,
            eframe::wgpu::DeviceType::VirtualGpu => 2,
            eframe::wgpu::DeviceType::Cpu => 3,
            eframe::wgpu::DeviceType::Other => 4,
        })
        .cloned()
        .ok_or_else(|| "no Direct3D 12 adapter supports the application window".to_owned())?;

    let available = adapters
        .iter()
        .map(|adapter| {
            let info = adapter.get_info();
            format!(
                "{} | {:?} | {:?}",
                info.name, info.device_type, info.backend
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let selected_info = selected.get_info();
    let report = format!(
        "Available Direct3D 12 adapters:\n{available}\n\nSelected:\n{} | {:?} | {:?}\n",
        selected_info.name, selected_info.device_type, selected_info.backend
    );
    let _ = fs::write(
        configuration_path().with_file_name("renderer-adapter.log"),
        report,
    );

    Ok(selected)
}

fn configuration_path() -> PathBuf {
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--config"
            && let Some(path) = arguments.next()
        {
            return PathBuf::from(path);
        }
    }
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("config.toml")
}
