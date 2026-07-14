use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use eframe::egui::{self, Color32, FontFamily, FontId, Margin, RichText, Stroke, TextStyle, Vec2};

use crate::{
    config::{AgreementType, AppConfig, CalInstallMethod, LicensingMode},
    i18n::{Language, Texts, texts},
    runner::{
        self, BaseAccessSelection, Feature, RdsRoleStatus, RoleInstallState, Task, TaskEvent,
    },
};

const PRIMARY: Color32 = Color32::from_rgb(37, 99, 235);
const PRIMARY_HOVER: Color32 = Color32::from_rgb(29, 78, 216);
const PRIMARY_SOFT: Color32 = Color32::from_rgb(239, 246, 255);
const NAVY: Color32 = Color32::from_rgb(15, 35, 63);
const BACKGROUND: Color32 = Color32::from_rgb(244, 247, 251);
const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
const BORDER: Color32 = Color32::from_rgb(220, 227, 237);
const TEXT: Color32 = Color32::from_rgb(24, 35, 52);
const MUTED: Color32 = Color32::from_rgb(100, 116, 139);
const SUCCESS: Color32 = Color32::from_rgb(5, 150, 105);
const SUCCESS_SOFT: Color32 = Color32::from_rgb(236, 253, 245);
const WARNING: Color32 = Color32::from_rgb(217, 119, 6);
const WARNING_SOFT: Color32 = Color32::from_rgb(255, 251, 235);
const ERROR: Color32 = Color32::from_rgb(220, 38, 38);

pub fn configure_style(ctx: &egui::Context) {
    install_cjk_font(ctx);
    ctx.set_theme(egui::Theme::Light);
    let mut visuals = egui::Visuals::light();
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.faint_bg_color = Color32::from_rgb(248, 250, 252);
    visuals.extreme_bg_color = SURFACE;
    visuals.text_edit_bg_color = Some(SURFACE);
    visuals.selection.bg_fill = PRIMARY;
    visuals.selection.stroke = Stroke::new(1.0, SURFACE);
    visuals.hyperlink_color = PRIMARY;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.weak_bg_fill = SURFACE;
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.weak_bg_fill = PRIMARY_SOFT;
    visuals.widgets.hovered.bg_fill = PRIMARY_SOFT;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, PRIMARY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, PRIMARY_HOVER);
    visuals.widgets.active.weak_bg_fill = PRIMARY_SOFT;
    visuals.widgets.active.bg_fill = PRIMARY_SOFT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, PRIMARY_HOVER);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, PRIMARY_HOVER);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(7);
    }
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(25.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        );
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(14.0, 8.0);
        style.spacing.interact_size.y = 34.0;
        style.spacing.text_edit_width = 320.0;
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    });
}

fn install_cjk_font(_ctx: &egui::Context) {
    #[cfg(target_os = "windows")]
    {
        let font_path = [
            r"C:\Windows\Fonts\simhei.ttf",
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\simsun.ttc",
        ]
        .into_iter()
        .find(|path| std::path::Path::new(path).exists());
        let Some(font_path) = font_path else {
            return;
        };
        let Ok(font_bytes) = std::fs::read(font_path) else {
            return;
        };
        let mut fonts = egui::FontDefinitions::default();
        let name = "windows-cjk".to_owned();
        fonts.font_data.insert(
            name.clone(),
            std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
        );
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            if let Some(font_names) = fonts.families.get_mut(&family) {
                font_names.push(name.clone());
            }
        }
        _ctx.set_fonts(fonts);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppStatus {
    Ready,
    Running,
    Completed,
    Failed,
    ConfigurationSaved,
    SaveFailed,
}

enum RdsRoleCheckState {
    Checking,
    Loaded(RdsRoleStatus),
    Failed(String),
}

impl AppStatus {
    fn label(self, text: &Texts) -> &'static str {
        match self {
            Self::Ready => text.ready,
            Self::Running => text.running,
            Self::Completed => text.completed,
            Self::Failed => text.failed,
            Self::ConfigurationSaved => text.configuration_saved,
            Self::SaveFailed => text.save_failed,
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::Completed | Self::ConfigurationSaved => SUCCESS,
            Self::Failed | Self::SaveFailed => ERROR,
            Self::Ready | Self::Running => MUTED,
        }
    }
}

fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(SURFACE).strong())
            .fill(PRIMARY)
            .stroke(Stroke::new(1.0, PRIMARY))
            .corner_radius(8)
            .min_size(Vec2::new(190.0, 40.0)),
    )
}

fn callout(ui: &mut egui::Ui, text: &str, warning: bool) {
    let (fill, color) = if warning {
        (WARNING_SOFT, WARNING)
    } else {
        (PRIMARY_SOFT, PRIMARY)
    };
    let width = ui.available_width();
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.35)))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(13, 10))
        .show(ui, |ui| {
            ui.set_min_width((width - 28.0).max(0.0));
            ui.label(RichText::new(text).color(color));
        });
}

fn role_state_label(text: &Texts, state: RoleInstallState) -> (&'static str, Color32) {
    match state {
        RoleInstallState::Installed => (text.role_installed, SUCCESS),
        RoleInstallState::PendingRestart => (text.role_pending_restart, WARNING),
        RoleInstallState::Missing => (text.role_missing, ERROR),
    }
}

pub struct SetupApp {
    config: AppConfig,
    config_path: PathBuf,
    selected_feature: Feature,
    base_access: BaseAccessSelection,
    remote_addresses: String,
    dry_run: bool,
    show_secrets: bool,
    running: bool,
    active_feature: Option<Feature>,
    events: Option<Receiver<TaskEvent>>,
    logs: Vec<String>,
    status: AppStatus,
    reboot_available: bool,
    show_log: bool,
    scroll_log_to_bottom: bool,
    rds_role_check_events: Option<Receiver<Result<RdsRoleStatus, String>>>,
    rds_role_check_state: RdsRoleCheckState,
}

impl SetupApp {
    pub fn new(config: AppConfig, config_path: PathBuf, startup_message: Option<String>) -> Self {
        let mut logs = vec![format!("Configuration: {}", config_path.display())];
        if let Some(message) = startup_message {
            logs.push(message);
        }
        let remote_addresses = config.base.allowed_remote_addresses.join(", ");
        let (rds_role_check_tx, rds_role_check_rx) = mpsc::channel();
        runner::spawn_rds_role_check(rds_role_check_tx);
        Self {
            config,
            config_path,
            selected_feature: Feature::BaseAccess,
            base_access: BaseAccessSelection::default(),
            remote_addresses,
            dry_run: !cfg!(target_os = "windows"),
            show_secrets: false,
            running: false,
            active_feature: None,
            events: None,
            logs,
            status: AppStatus::Ready,
            reboot_available: false,
            show_log: true,
            scroll_log_to_bottom: true,
            rds_role_check_events: Some(rds_role_check_rx),
            rds_role_check_state: RdsRoleCheckState::Checking,
        }
    }

    fn refresh_rds_role_status(&mut self) {
        let (tx, rx) = mpsc::channel();
        runner::spawn_rds_role_check(tx);
        self.rds_role_check_events = Some(rx);
        self.rds_role_check_state = RdsRoleCheckState::Checking;
    }

    fn poll_rds_role_status(&mut self) {
        let Some(rx) = self.rds_role_check_events.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(status)) => {
                if status.has_pending_restart() {
                    self.reboot_available = true;
                }
                self.rds_role_check_state = RdsRoleCheckState::Loaded(status);
            }
            Ok(Err(error)) => {
                self.rds_role_check_state = RdsRoleCheckState::Failed(error);
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.rds_role_check_events = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.rds_role_check_state =
                    RdsRoleCheckState::Failed("RDS role detection stopped unexpectedly".to_owned());
            }
        }
    }

    fn save_config(&mut self) {
        self.sync_remote_addresses();
        match self.config.save(&self.config_path) {
            Ok(()) => {
                self.status = AppStatus::ConfigurationSaved;
                self.logs
                    .push(format!("Saved {}", self.config_path.display()));
                self.scroll_log_to_bottom = true;
            }
            Err(error) => {
                self.status = AppStatus::SaveFailed;
                self.logs.push(format!("ERROR: {error:#}"));
                self.scroll_log_to_bottom = true;
            }
        }
    }

    fn sync_remote_addresses(&mut self) {
        self.config.base.allowed_remote_addresses = self
            .remote_addresses
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect();
    }

    fn start(&mut self, task: Task, feature: Option<Feature>) {
        if self.running {
            return;
        }
        self.sync_remote_addresses();
        if let Err(error) = self.config.save(&self.config_path) {
            self.logs
                .push(format!("ERROR: unable to save configuration: {error:#}"));
            return;
        }
        let (tx, rx) = mpsc::channel();
        runner::spawn_task(task, self.config.clone(), self.dry_run, tx);
        self.events = Some(rx);
        self.running = true;
        self.show_log = true;
        self.scroll_log_to_bottom = true;
        self.active_feature = feature;
        self.status = AppStatus::Running;
    }

    fn poll_events(&mut self) {
        let Some(rx) = self.events.take() else {
            return;
        };
        let mut finished = false;
        let mut refresh_rds_roles = false;
        while let Ok(event) = rx.try_recv() {
            self.scroll_log_to_bottom = true;
            match event {
                TaskEvent::Log(message) => self.logs.push(message),
                TaskEvent::Finished(result) => {
                    finished = true;
                    self.running = false;
                    refresh_rds_roles = self.active_feature == Some(Feature::RdsRoles);
                    match result {
                        Ok(outcome) => {
                            self.status = AppStatus::Completed;
                            self.logs.push("Task completed successfully.".to_owned());
                            if outcome.reboot_required {
                                self.reboot_available = true;
                                self.logs.push(
                                    "Restart Windows to finish applying the installed roles."
                                        .to_owned(),
                                );
                            }
                            if let Some(path) = outcome.credential_file {
                                self.logs.push(format!("Credential file: {path}"));
                            }
                        }
                        Err(error) => {
                            self.status = AppStatus::Failed;
                            self.logs.push(format!("ERROR: {error}"));
                        }
                    }
                    self.active_feature = None;
                }
            }
        }
        if !finished {
            self.events = Some(rx);
        }
        if refresh_rds_roles {
            self.refresh_rds_role_status();
        }
    }

    fn feature_navigation(&mut self, ui: &mut egui::Ui, text: &Texts) {
        let gap = 6.0;
        let tab_width = ((ui.available_width() - gap * 3.0) / 4.0).max(130.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for feature in Feature::ALL {
                let selected = self.selected_feature == feature;
                let (fill, stroke, color) = if selected {
                    (PRIMARY, Stroke::new(1.0, PRIMARY), SURFACE)
                } else {
                    (BACKGROUND, Stroke::new(1.0, BORDER), TEXT)
                };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(text.feature_title(feature))
                                .size(14.0)
                                .strong()
                                .color(color),
                        )
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(8)
                        .min_size(Vec2::new(tab_width, 42.0)),
                    )
                    .on_hover_text(text.feature_description(feature))
                    .clicked()
                {
                    self.selected_feature = feature;
                }
            }
        });
    }

    fn feature_content(&mut self, ui: &mut egui::Ui, text: &Texts) {
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(PRIMARY_SOFT)
                .corner_radius(7)
                .inner_margin(Margin::symmetric(9, 5))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text.function_badge)
                            .size(11.0)
                            .strong()
                            .color(PRIMARY),
                    );
                });
            if self.running && self.active_feature == Some(self.selected_feature) {
                ui.spinner();
            }
        });
        ui.add_space(5.0);
        ui.heading(text.feature_title(self.selected_feature));
        ui.label(
            RichText::new(text.feature_description(self.selected_feature))
                .size(14.0)
                .color(MUTED),
        );
        ui.add_space(18.0);

        ui.add_enabled_ui(!self.running, |ui| match self.selected_feature {
            Feature::BaseAccess => self.base_access_ui(ui, text),
            Feature::RdsRoles => self.rds_roles_ui(ui, text),
            Feature::RdsLicensing => self.rds_licensing_ui(ui, text),
            Feature::LocalUsers => self.local_users_ui(ui, text),
        });
    }

    fn base_access_ui(&mut self, ui: &mut egui::Ui, text: &Texts) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.base_access.kms, text.kms_activation);
            ui.checkbox(&mut self.base_access.ssh, text.openssh);
            if self.base_access.ssh {
                ui.colored_label(PRIMARY, text.tcp_22_linked);
            }
            ui.checkbox(&mut self.base_access.rdp, text.remote_desktop);
            if self.base_access.rdp {
                ui.colored_label(PRIMARY, text.tcp_3389_linked);
            }
        });
        ui.small(text.unchecked_actions);
        ui.add_space(12.0);

        egui::Grid::new("base-grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(text.windows_product_key);
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.base.windows_product_key)
                        .password(!self.show_secrets)
                        .desired_width(360.0),
                );
                ui.end_row();
                ui.label(text.kms_server);
                ui.text_edit_singleline(&mut self.config.base.kms_server);
                ui.end_row();
                ui.label(text.firewall_sources);
                ui.text_edit_singleline(&mut self.remote_addresses);
                ui.end_row();
            });
        ui.label(text.administrator_ssh_key);
        ui.add(
            egui::TextEdit::multiline(&mut self.config.base.administrator_public_key)
                .desired_width(f32::INFINITY)
                .desired_rows(3),
        );
        ui.add_space(12.0);
        if primary_button(ui, text.run_selected_actions).clicked() {
            self.start(
                Task::BaseAccess(self.base_access),
                Some(Feature::BaseAccess),
            );
        }
    }

    fn rds_roles_ui(&mut self, ui: &mut egui::Ui, text: &Texts) {
        ui.label(RichText::new(text.rds_role_status).strong().color(TEXT));
        ui.add_space(6.0);
        match &self.rds_role_check_state {
            RdsRoleCheckState::Checking => {
                egui::Frame::new()
                    .fill(PRIMARY_SOFT)
                    .stroke(Stroke::new(1.0, PRIMARY.gamma_multiply(0.35)))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(13, 11))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(text.checking_rds_roles).color(PRIMARY));
                        });
                    });
            }
            RdsRoleCheckState::Loaded(status) => {
                let (summary, fill, color) = if status.fully_installed() {
                    (text.rds_roles_installed, SUCCESS_SOFT, SUCCESS)
                } else if status.all_present() && status.has_pending_restart() {
                    (text.rds_roles_pending_restart, WARNING_SOFT, WARNING)
                } else {
                    (text.rds_roles_incomplete, WARNING_SOFT, WARNING)
                };
                egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0, color.gamma_multiply(0.35)))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(13, 11))
                    .show(ui, |ui| {
                        ui.label(RichText::new(summary).strong().color(color));
                        ui.add_space(7.0);
                        egui::Grid::new("rds-role-status-grid")
                            .num_columns(2)
                            .spacing([24.0, 6.0])
                            .show(ui, |ui| {
                                for (label, state) in [
                                    (text.rd_session_host, status.session_host),
                                    (text.rd_licensing, status.licensing),
                                    (text.rd_licensing_tools, status.licensing_ui),
                                ] {
                                    let (state_label, state_color) = role_state_label(text, state);
                                    ui.label(label);
                                    ui.colored_label(state_color, state_label);
                                    ui.end_row();
                                }
                            });
                    });
            }
            RdsRoleCheckState::Failed(error) => {
                callout(
                    ui,
                    &format!("{}: {error}", text.rds_role_check_failed),
                    true,
                );
            }
        }
        ui.add_space(12.0);
        callout(ui, text.install_rds_warning, true);
        ui.label(text.restart_explanation);
        ui.add_space(16.0);
        let can_install = match &self.rds_role_check_state {
            RdsRoleCheckState::Checking => false,
            RdsRoleCheckState::Loaded(status) => !status.all_present(),
            RdsRoleCheckState::Failed(_) => true,
        };
        let mut install_clicked = false;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(can_install, |ui| {
                install_clicked = primary_button(ui, text.install_rds_roles).clicked();
            });
            let can_recheck = !matches!(&self.rds_role_check_state, RdsRoleCheckState::Checking);
            if ui
                .add_enabled(can_recheck, egui::Button::new(text.recheck_rds_roles))
                .clicked()
            {
                self.refresh_rds_role_status();
            }
        });
        if install_clicked {
            self.start(Task::RdsRoles, Some(Feature::RdsRoles));
        }
        if self.reboot_available {
            ui.add_space(10.0);
            if ui
                .add(
                    egui::Button::new(RichText::new(text.restart_windows).color(Color32::WHITE))
                        .fill(WARNING),
                )
                .clicked()
            {
                self.start(Task::Reboot, None);
            }
        }
    }

    fn rds_licensing_ui(&mut self, ui: &mut egui::Ui, text: &Texts) {
        callout(ui, text.entitlement_warning, true);
        ui.add_space(10.0);
        ui.label(
            RichText::new(text.server_environment)
                .size(15.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(5.0);
        let domain_membership = match &self.rds_role_check_state {
            RdsRoleCheckState::Checking => {
                callout(ui, text.checking_server_environment, false);
                None
            }
            RdsRoleCheckState::Loaded(status) if status.part_of_domain => {
                callout(ui, text.domain_environment, false);
                Some(true)
            }
            RdsRoleCheckState::Loaded(_) => {
                callout(
                    ui,
                    &format!(
                        "{} {}",
                        text.workgroup_environment, text.workgroup_per_user_warning
                    ),
                    true,
                );
                Some(false)
            }
            RdsRoleCheckState::Failed(error) => {
                callout(
                    ui,
                    &format!("{}: {error}", text.rds_role_check_failed),
                    true,
                );
                None
            }
        };
        if ui
            .add_enabled(
                domain_membership.is_some(),
                egui::Button::new(text.apply_recommended_settings)
                    .fill(PRIMARY_SOFT)
                    .stroke(Stroke::new(1.0, PRIMARY))
                    .corner_radius(7),
            )
            .clicked()
        {
            self.config.rds.licensing_mode = if domain_membership == Some(true) {
                LicensingMode::PerUser
            } else {
                LicensingMode::PerDevice
            };
            self.config.rds.license_server = "localhost".to_owned();
            self.config.rds.cal_install_method = CalInstallMethod::Agreement;
            self.config.rds.agreement_type = AgreementType::Enterprise;
            self.config.rds.cal_product_version_id = 7;
        }

        ui.add_space(18.0);
        ui.label(
            RichText::new(text.licensing_policy_section)
                .size(15.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(6.0);
        egui::Grid::new("rds-policy-grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label(text.licensing_mode);
                egui::ComboBox::from_id_salt("licensing-mode")
                    .selected_text(text.licensing_mode_label(self.config.rds.licensing_mode))
                    .show_ui(ui, |ui| {
                        for value in LicensingMode::ALL {
                            ui.selectable_value(
                                &mut self.config.rds.licensing_mode,
                                value,
                                text.licensing_mode_label(value),
                            );
                        }
                    });
                ui.end_row();
                ui.label(text.license_server);
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.rds.license_server)
                        .desired_width(300.0),
                );
                ui.end_row();
            });
        ui.label(RichText::new(text.license_server_help).small().color(MUTED));
        if domain_membership == Some(false)
            && self.config.rds.licensing_mode == LicensingMode::PerUser
        {
            ui.colored_label(WARNING, text.workgroup_per_user_warning);
        }

        ui.add_space(18.0);
        ui.label(
            RichText::new(text.activate_server_section)
                .size(15.0)
                .strong()
                .color(TEXT),
        );
        ui.label(
            RichText::new(text.automatic_connection)
                .small()
                .color(MUTED),
        );
        ui.add_space(7.0);
        ui.label(RichText::new(text.required_contact).strong());
        {
            let contact = &mut self.config.rds.contact;
            egui::Grid::new("required-contact-grid")
                .num_columns(4)
                .spacing([10.0, 7.0])
                .show(ui, |ui| {
                    ui.label(text.first_name);
                    ui.text_edit_singleline(&mut contact.first_name);
                    ui.label(text.last_name);
                    ui.text_edit_singleline(&mut contact.last_name);
                    ui.end_row();
                    ui.label(text.company);
                    ui.text_edit_singleline(&mut contact.company);
                    ui.label(text.country);
                    ui.text_edit_singleline(&mut contact.country_region);
                    ui.end_row();
                });
        }
        ui.collapsing(text.optional_contact, |ui| {
            let contact = &mut self.config.rds.contact;
            egui::Grid::new("optional-contact-grid")
                .num_columns(4)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label(text.email);
                    ui.text_edit_singleline(&mut contact.email);
                    ui.label(text.org_unit);
                    ui.text_edit_singleline(&mut contact.org_unit);
                    ui.end_row();
                    ui.label(text.address);
                    ui.text_edit_singleline(&mut contact.address);
                    ui.label(text.city);
                    ui.text_edit_singleline(&mut contact.city);
                    ui.end_row();
                    ui.label(text.state);
                    ui.text_edit_singleline(&mut contact.state);
                    ui.label(text.postal_code);
                    ui.text_edit_singleline(&mut contact.postal_code);
                    ui.end_row();
                });
        });

        ui.add_space(18.0);
        ui.label(
            RichText::new(text.install_cal_section)
                .size(15.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(text.cal_method);
            egui::ComboBox::from_id_salt("cal-method")
                .selected_text(text.cal_method_label(self.config.rds.cal_install_method))
                .show_ui(ui, |ui| {
                    for value in CalInstallMethod::ALL {
                        ui.selectable_value(
                            &mut self.config.rds.cal_install_method,
                            value,
                            text.cal_method_label(value),
                        );
                    }
                });
        });

        match self.config.rds.cal_install_method {
            CalInstallMethod::Agreement => {
                ui.label(
                    RichText::new(text.license_program_help)
                        .small()
                        .color(MUTED),
                );
                egui::Grid::new("agreement-cal-grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(text.license_program);
                        egui::ComboBox::from_id_salt("agreement-type")
                            .selected_text(self.config.rds.agreement_type.as_str())
                            .show_ui(ui, |ui| {
                                for value in AgreementType::ALL {
                                    ui.selectable_value(
                                        &mut self.config.rds.agreement_type,
                                        value,
                                        value.as_str(),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label(text.agreement_number);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.rds.agreement_number)
                                .password(!self.show_secrets)
                                .desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label(text.product_version);
                        ui.label(text.windows_server_2022_cal);
                        ui.end_row();
                        ui.label(text.cal_license_type);
                        ui.label(text.licensing_mode_label(self.config.rds.licensing_mode));
                        ui.end_row();
                        ui.label(text.purchased_cal_count);
                        ui.add(
                            egui::DragValue::new(&mut self.config.rds.cal_count).range(1..=10_000),
                        );
                        ui.end_row();
                    });
                ui.label(
                    RichText::new(text.agreement_number_help)
                        .small()
                        .color(MUTED),
                );
                ui.label(RichText::new(text.purchased_cal_help).small().color(MUTED));
                self.config.rds.cal_product_version_id = 7;
            }
            CalInstallMethod::KeyPackId => {
                ui.horizontal(|ui| {
                    ui.label(text.key_pack_id);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.config.rds.license_key_pack_id)
                            .password(!self.show_secrets)
                            .desired_width(330.0),
                    );
                });
            }
            CalInstallMethod::None => {
                ui.colored_label(WARNING, text.choose_cal_method);
            }
        }
        ui.add_space(12.0);
        if primary_button(ui, text.activate_rds_licensing).clicked() {
            self.start(Task::RdsLicensing, Some(Feature::RdsLicensing));
        }
    }

    fn local_users_ui(&mut self, ui: &mut egui::Ui, text: &Texts) {
        ui.label(text.rdp_group_explanation);
        egui::Grid::new("users-grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(text.user_count);
                ui.add(egui::DragValue::new(&mut self.config.users.count).range(1..=200));
                ui.end_row();
                ui.label(text.prefix);
                ui.text_edit_singleline(&mut self.config.users.prefix);
                ui.end_row();
                ui.label(text.start_index);
                ui.add(egui::DragValue::new(&mut self.config.users.start_index).range(0..=99_999));
                ui.end_row();
                ui.label(text.number_width);
                ui.add(egui::DragValue::new(&mut self.config.users.number_width).range(1..=6));
                ui.end_row();
                ui.label(text.password_length);
                ui.add(egui::DragValue::new(&mut self.config.users.password_length).range(14..=64));
                ui.end_row();
            });
        ui.checkbox(
            &mut self.config.users.reset_existing_passwords,
            text.reset_existing_passwords,
        );
        callout(ui, text.credential_warning, true);
        ui.add_space(12.0);
        if primary_button(ui, text.create_users).clicked() {
            self.start(Task::LocalUsers, Some(Feature::LocalUsers));
        }
    }

    fn log_panel(&mut self, ui: &mut egui::Ui, text: &Texts, log_content_height: f32) {
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!(
                            "{}  {}",
                            text.audit_log,
                            if self.show_log { "–" } else { "+" }
                        ))
                        .strong()
                        .color(TEXT),
                    )
                    .frame(false),
                )
                .clicked()
            {
                self.show_log = !self.show_log;
                if self.show_log {
                    self.scroll_log_to_bottom = true;
                }
            }
            ui.label(RichText::new(self.status.label(text)).color(self.status.color()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(text.clear_log).small().corner_radius(6))
                    .clicked()
                {
                    self.logs.clear();
                }
            });
        });
        if self.show_log {
            let force_bottom = self.scroll_log_to_bottom;
            ui.add_space(5.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(248, 250, 252))
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(8)
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(
                        (ui.available_width() - 20.0).max(0.0),
                        log_content_height,
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("audit-log-scroll")
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .max_height(log_content_height)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            for line in &self.logs {
                                let color = if line.starts_with("ERROR:") {
                                    ERROR
                                } else {
                                    MUTED
                                };
                                ui.label(RichText::new(line).monospace().size(12.0).color(color));
                            }
                            if force_bottom {
                                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                            }
                        });
                });
            self.scroll_log_to_bottom = false;
        }
    }
}

impl eframe::App for SetupApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_events();
        self.poll_rds_role_status();
        if self.running || matches!(&self.rds_role_check_state, RdsRoleCheckState::Checking) {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let text = texts(self.config.ui.language);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(text.app_title.to_owned()));

        let outer_width = ui.available_width();
        egui::Frame::new()
            .fill(BACKGROUND)
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                let root_width = (outer_width - 28.0).max(0.0);
                ui.set_min_width(root_width);
                egui::Frame::new()
                    .fill(NAVY)
                    .corner_radius(11)
                    .inner_margin(Margin::symmetric(20, 14))
                    .show(ui, |ui| {
                        ui.set_min_width((root_width - 40.0).max(0.0));
                        let mut language_changed = false;
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(text.app_title)
                                        .size(22.0)
                                        .strong()
                                        .color(SURFACE),
                                );
                                ui.label(
                                    RichText::new(text.app_subtitle)
                                        .size(12.5)
                                        .color(Color32::from_rgb(183, 202, 226)),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(text.save_configuration)
                                                    .color(PRIMARY_HOVER)
                                                    .strong(),
                                            )
                                            .fill(SURFACE)
                                            .stroke(Stroke::NONE)
                                            .corner_radius(8),
                                        )
                                        .clicked()
                                    {
                                        self.save_config();
                                    }
                                    ui.checkbox(
                                        &mut self.show_secrets,
                                        RichText::new(text.show_secrets).color(SURFACE),
                                    );
                                    ui.checkbox(
                                        &mut self.dry_run,
                                        RichText::new(text.dry_run).color(SURFACE),
                                    );
                                    egui::ComboBox::from_id_salt("language-selector")
                                        .selected_text(self.config.ui.language.native_name())
                                        .width(92.0)
                                        .show_ui(ui, |ui| {
                                            for language in Language::ALL {
                                                language_changed |= ui
                                                    .selectable_value(
                                                        &mut self.config.ui.language,
                                                        language,
                                                        language.native_name(),
                                                    )
                                                    .changed();
                                            }
                                        });
                                    ui.label(RichText::new(text.language).color(SURFACE));
                                },
                            );
                        });
                        if language_changed {
                            self.save_config();
                        }
                    });

                ui.add_space(12.0);
                egui::Frame::new()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(11)
                    .inner_margin(Margin::same(7))
                    .show(ui, |ui| {
                        ui.set_min_width((root_width - 16.0).max(0.0));
                        self.feature_navigation(ui, text);
                    });
                ui.add_space(10.0);
                let body_height = ui.available_height().max(420.0);
                let log_width = (root_width * 0.35).clamp(370.0, 440.0);
                let column_gap = 10.0;
                let content_width = (root_width - log_width - column_gap).max(500.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = column_gap;
                    let content_size = Vec2::new(content_width, body_height);
                    ui.allocate_ui_with_layout(
                        content_size,
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(SURFACE)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(11)
                                .inner_margin(Margin::symmetric(22, 18))
                                .show(ui, |ui| {
                                    ui.set_min_size(Vec2::new(
                                        (content_size.x - 46.0).max(0.0),
                                        body_height - 38.0,
                                    ));
                                    egui::ScrollArea::vertical()
                                        .id_salt(("feature-content-scroll", self.selected_feature))
                                        .show(ui, |ui| {
                                            self.feature_content(ui, text);
                                        });
                                });
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(log_width, body_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(SURFACE)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(11)
                                .inner_margin(Margin::symmetric(14, 12))
                                .show(ui, |ui| {
                                    ui.set_min_size(Vec2::new(
                                        (log_width - 30.0).max(0.0),
                                        body_height - 26.0,
                                    ));
                                    self.log_panel(ui, text, (body_height - 92.0).max(240.0));
                                });
                        },
                    );
                });
            });
    }
}
