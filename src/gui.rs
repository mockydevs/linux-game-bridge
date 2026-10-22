use eframe::egui::{self, Color32, RichText};
use linux_game_bridge::engine::{self, Doctor, Options};
use std::{
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Action {
    Prepare,
    Install,
    Update,
    Play,
    Launcher,
    Shortcut,
}
impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Prepare => "Preparing Steam",
            Self::Install => "Installing TruckersMP",
            Self::Update => "Verifying updates",
            Self::Play => "Starting TruckersMP",
            Self::Launcher => "Opening official launcher",
            Self::Shortcut => "Adding application shortcut",
        }
    }
}
enum Event {
    Log(String),
    Done(Result<(), String>),
}

pub struct App {
    options: Options,
    status: Result<Doctor, String>,
    receiver: Option<Receiver<Event>>,
    activity: String,
    error: Option<String>,
    logs: Vec<String>,
    last_scan: Instant,
    steam_folder: String,
    smoke_frames: Option<u32>,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options, smoke: bool) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        style.spacing.button_padding = egui::vec2(16.0, 10.0);
        style.visuals.panel_fill = Color32::from_rgb(17, 24, 33);
        style.visuals.selection.bg_fill = Color32::from_rgb(22, 101, 82);
        cc.egui_ctx.set_style(style);
        let status = engine::doctor(&options).map_err(|e| format!("{e:#}"));
        let steam_folder = options
            .steam_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self {
            options,
            status,
            receiver: None,
            activity: "Ready when you are".into(),
            error: None,
            logs: vec![],
            last_scan: Instant::now(),
            steam_folder,
            smoke_frames: smoke.then_some(0),
        }
    }
    fn scan(&mut self) {
        self.status = engine::doctor(&self.options).map_err(|e| format!("{e:#}"));
        self.last_scan = Instant::now();
    }
    fn start(&mut self, action: Action, ctx: &egui::Context) {
        if self.receiver.is_some() {
            return;
        }
        self.error = None;
        self.activity = action.label().into();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        let options = self.options.clone();
        let context = ctx.clone();
        std::thread::spawn(move || {
            let progress = |message: String| {
                let _ = tx.send(Event::Log(message));
                context.request_repaint();
            };
            let result = match action {
                Action::Prepare => engine::prepare(&options, &progress),
                Action::Install => engine::install(&options, &progress),
                Action::Update => engine::update(&options, &progress),
                Action::Play => engine::play(&options, false, &progress),
                Action::Launcher => engine::play(&options, true, &progress),
                Action::Shortcut => engine::install_desktop(&options)
                    .map(|_| progress("Linux Game Bridge added to your applications menu".into())),
            };
            let _ = tx.send(Event::Done(result.map_err(|e| format!("{e:#}"))));
            context.request_repaint();
        });
    }
    fn poll(&mut self) {
        let events: Vec<_> = self
            .receiver
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Log(message) => {
                    self.activity = message.clone();
                    self.logs.push(message);
                    if self.logs.len() > 150 {
                        self.logs.remove(0);
                    }
                }
                Event::Done(result) => {
                    self.receiver = None;
                    match result {
                        Ok(()) => self.activity = "Finished. Check the steps below.".into(),
                        Err(e) => {
                            self.activity = "Needs attention".into();
                            self.error = Some(e);
                        }
                    }
                    self.scan();
                }
            }
        }
    }
}
fn row(ui: &mut egui::Ui, ready: bool, title: &str, detail: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(if ready { "●" } else { "○" }).color(if ready {
                Color32::from_rgb(92, 219, 164)
            } else {
                Color32::from_rgb(246, 187, 96)
            }),
        );
        ui.vertical(|ui| {
            ui.strong(title);
            ui.label(RichText::new(detail).small().color(Color32::GRAY));
        });
    });
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.receiver.is_none() && self.last_scan.elapsed() > Duration::from_secs(4) {
            self.scan();
        }
        ctx.request_repaint_after(Duration::from_millis(300));
        if let Some(frames) = &mut self.smoke_frames {
            *frames += 1;
            if *frames > 8 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("LINUX GAME BRIDGE").size(13.0).color(Color32::from_rgb(92, 219, 164)).strong());
                ui.heading(RichText::new("Your next drive starts here.").size(30.0));
                ui.label("Set up the official TruckersMP client on Linux, then keep it ready to play.");
                ui.add_space(10.0);
                let busy = self.receiver.is_some();
                egui::Frame::group(ui.style()).inner_margin(18.0).show(ui, |ui| {
                    match &self.status {
                        Ok(status) => {
                            row(ui, true, "Steam found", &status.steam.root.display().to_string());
                            row(ui, status.steam.ready(), "Windows ETS2 + Proton", if status.steam.ready() { "Required game files and compatibility runtime are installed" } else { "Prepare Steam, then let its downloads finish" });
                            row(ui, status.launcher_installed && status.prefix_initialized && status.mod_installed, "Official TruckersMP client", if status.launcher_installed && status.mod_installed { "Installed · files checked again before each launch" } else { "Install after Steam finishes preparing the game" });
                            if status.steam.game_running { ui.label(RichText::new("ETS2 is running. Close it before installing or updating.").color(Color32::from_rgb(246, 187, 96))); }
                        }
                        Err(error) => { ui.label(RichText::new(error).color(Color32::from_rgb(246, 187, 96))); ui.hyperlink_to("Get Steam", "https://store.steampowered.com/about/"); }
                    }
                });
                ui.add_space(6.0);
                ui.strong("1. Prepare Steam");
                ui.label("Requires your own Steam copy of Euro Truck Simulator 2. This backs up Steam settings, enables Proton Experimental for ETS2, and starts Steam's downloads. If needed, Steam will close and reopen. Complete its sign-in or installation prompts; close ETS2 after its first launch.");
                let found = self.status.is_ok();
                let game_running = self.status.as_ref().is_ok_and(|s| s.steam.game_running);
                if ui.add_enabled(!busy && found && !game_running, egui::Button::new("Prepare Steam")).clicked() { self.start(Action::Prepare, ctx); }
                ui.separator();
                ui.strong("2. Install TruckersMP");
                ui.label("Downloads the official launcher and multiplayer files, verifies them, and connects them to your Steam game. Your account details stay in Steam and TruckersMP.");
                let ready = self.status.as_ref().is_ok_and(|s| s.steam.ready());
                if ui.add_enabled(!busy && ready && !game_running, egui::Button::new("Install / finish setup")).clicked() { self.start(Action::Install, ctx); }
                ui.separator();
                ui.strong("3. Play together");
                ui.label("Every launch checks for current multiplayer files. Sign in inside TruckersMP and choose an official server shared with Windows players.");
                let installed = self.status.as_ref().is_ok_and(|s| s.launcher_installed && s.prefix_initialized && s.mod_installed);
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!busy && ready && installed && !game_running, egui::Button::new(RichText::new("Play TruckersMP").strong()).fill(Color32::from_rgb(24, 109, 84))).clicked() { self.start(Action::Play, ctx); }
                    if ui.add_enabled(!busy && installed && !game_running, egui::Button::new("Official launcher")).clicked() { self.start(Action::Launcher, ctx); }
                    if ui.add_enabled(!busy && installed && !game_running, egui::Button::new("Verify / repair mod")).clicked() { self.start(Action::Update, ctx); }
                });
                ui.add_space(6.0);
                if busy { ui.horizontal(|ui| { ui.spinner(); ui.label(&self.activity); }); } else { ui.label(&self.activity); }
                if let Some(error) = &self.error { ui.colored_label(Color32::from_rgb(255, 145, 132), error); }
                egui::CollapsingHeader::new("Activity & troubleshooting").show(ui, |ui| {
                    for log in &self.logs { ui.label(log); }
                    if let Ok(status) = &self.status { for message in &status.messages { ui.label(message); } }
                    ui.label(format!("App files: {}", self.options.data.display()));
                    ui.label("If the official client rejects an ETS2 version, use Steam's beta branch recommended by TruckersMP. An app cannot bypass unsupported anti-cheat or server requirements.");
                    ui.hyperlink_to("TruckersMP support", "https://truckersmp.com/knowledge-base");
                });
                egui::CollapsingHeader::new("Application settings").show(ui, |ui| {
                    ui.label("Custom native Steam folder (leave empty for automatic detection)");
                    ui.text_edit_singleline(&mut self.steam_folder);
                    if ui.add_enabled(!busy, egui::Button::new("Use this Steam folder")).clicked() {
                        self.options.steam_root = if self.steam_folder.trim().is_empty() { None } else { Some(self.steam_folder.trim().into()) }; self.scan();
                    }
                    if ui.add_enabled(!busy, egui::Button::new("Add Linux Game Bridge to applications menu")).clicked() { self.start(Action::Shortcut, ctx); }
                });
                ui.separator();
                ui.label(RichText::new("Unofficial · Linux x86_64 · Native Steam required · Experimental Linux compatibility").small().color(Color32::GRAY));
                ui.label(RichText::new("Not affiliated with TruckersMP, SCS Software, or Valve. Windows games run through Proton; they are not native Linux ports.").small().color(Color32::GRAY));
            });
        });
    }
}
