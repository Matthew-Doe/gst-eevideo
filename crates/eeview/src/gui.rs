use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use eframe::egui::{self, ColorImage, TextureHandle};
use gsteevideo::eevideo_control::{
    CoapRegisterBackendConfig, ControlTarget, ControlTransportKind, DeviceController,
    DeviceSummary,
};

use crate::session::{
    ManagedTransportSettings, RecordingConfig, ViewerEvent, ViewerPipeline, ViewerSessionConfig,
    ViewerState, ViewerStats,
};

#[derive(Debug, Clone)]
pub struct RecordingForm {
    pub enabled: bool,
    pub path: PathBuf,
}

impl Default for RecordingForm {
    fn default() -> Self {
        Self {
            enabled: false,
            path: PathBuf::new(),
        }
    }
}

impl RecordingForm {
    pub fn to_recording_config(&self) -> Result<RecordingConfig> {
        if !self.enabled {
            bail!("recording is disabled");
        }
        if self.path.as_os_str().is_empty() {
            bail!("recording path is required");
        }
        Ok(RecordingConfig {
            path: self.path.clone(),
            encoder: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OperatorConsoleState {
    pub devices: Vec<DeviceSummary>,
    pub selected_device: Option<usize>,
    pub manual_device_uri: String,
    pub iface: String,
    pub bind_address: String,
    pub stream_name: String,
    pub port: u32,
    pub timeout_ms: u64,
    pub source_timeout_ms: u64,
    pub latency_ms: u64,
    pub max_packet_size: u16,
    pub packet_delay_ns: u64,
    pub recording: RecordingForm,
    pub locked: bool,
}

impl Default for OperatorConsoleState {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            selected_device: None,
            manual_device_uri: String::new(),
            iface: String::new(),
            bind_address: "127.0.0.1".to_string(),
            stream_name: "stream0".to_string(),
            port: 5000,
            timeout_ms: 1000,
            source_timeout_ms: 2000,
            latency_ms: 0,
            max_packet_size: 1400,
            packet_delay_ns: 0,
            recording: RecordingForm::default(),
            locked: false,
        }
    }
}

impl OperatorConsoleState {
    pub fn can_start(&self) -> bool {
        !self.locked
            && self.target().is_some()
            && !self.bind_address.trim().is_empty()
            && !self.stream_name.trim().is_empty()
            && (!self.recording.enabled || self.recording.to_recording_config().is_ok())
    }

    pub fn target(&self) -> Option<ControlTarget> {
        if let Some(index) = self.selected_device {
            if let Some(device) = self.devices.get(index) {
                return Some(device.target.clone());
            }
        }
        let device_uri = self.manual_device_uri.trim();
        if device_uri.is_empty() {
            None
        } else {
            Some(ControlTarget {
                device_uri: device_uri.to_string(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            })
        }
    }

    pub fn session_config(&self) -> Result<ViewerSessionConfig> {
        let target = self.target().ok_or_else(|| anyhow::anyhow!("device is required"))?;
        Ok(ViewerSessionConfig {
            target,
            bind_address: self.bind_address.clone(),
            port: self.port,
            source_timeout: Duration::from_millis(self.source_timeout_ms),
            latency: Duration::from_millis(self.latency_ms),
            stream_name: self.stream_name.clone(),
            managed_transport: ManagedTransportSettings {
                max_packet_size: self.max_packet_size,
                packet_delay_ns: self.packet_delay_ns,
            },
            recording: if self.recording.enabled {
                Some(self.recording.to_recording_config()?)
            } else {
                None
            },
            overlay_text: None,
        })
    }
}

enum WorkerCommand {
    Refresh(OperatorConsoleState),
    Start(ViewerSessionConfig, CoapRegisterBackendConfig),
    Stop,
}

enum WorkerEvent {
    Devices(Vec<DeviceSummary>),
    Viewer(ViewerEvent),
    Stopped(ViewerStats),
    Error(String),
}

pub struct OperatorConsoleApp {
    state: OperatorConsoleState,
    tx: Sender<WorkerCommand>,
    rx: Receiver<WorkerEvent>,
    texture: Option<TextureHandle>,
    viewer_state: ViewerState,
    stats: ViewerStats,
    frames: u64,
    last_error: Option<String>,
    last_frame_at: Option<Instant>,
}

impl OperatorConsoleApp {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        thread::spawn(move || worker_loop(command_rx, event_tx));

        Self {
            state: OperatorConsoleState::default(),
            tx: command_tx,
            rx: event_rx,
            texture: None,
            viewer_state: ViewerState::Stopped,
            stats: ViewerStats::default(),
            frames: 0,
            last_error: None,
            last_frame_at: None,
        }
    }

    fn refresh(&self) {
        self.tx
            .send(WorkerCommand::Refresh(self.state.clone()))
            .ok();
    }

    fn start(&mut self) {
        match self.state.session_config() {
            Ok(config) => {
                let backend_config = self.backend_config();
                self.state.locked = true;
                self.viewer_state = ViewerState::Starting;
                self.tx.send(WorkerCommand::Start(config, backend_config)).ok();
            }
            Err(err) => self.last_error = Some(format!("{err:#}")),
        }
    }

    fn stop(&self) {
        self.tx.send(WorkerCommand::Stop).ok();
    }

    fn backend_config(&self) -> CoapRegisterBackendConfig {
        CoapRegisterBackendConfig {
            interface_name: empty_to_none(&self.state.iface),
            bind_address: Some(self.state.bind_address.clone()),
            discovery_timeout: Duration::from_millis(self.state.timeout_ms),
            request_timeout: Duration::from_millis(self.state.timeout_ms),
            yaml_root: None,
            local_port: 0,
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                WorkerEvent::Devices(devices) => {
                    self.state.devices = devices;
                    self.state.selected_device = if self.state.devices.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                }
                WorkerEvent::Viewer(ViewerEvent::Frame(frame)) => {
                    if frame.width > 0 && frame.height > 0 {
                        let image = ColorImage::from_rgba_unmultiplied(
                            [frame.width as usize, frame.height as usize],
                            &frame.rgba,
                        );
                        let texture =
                            ctx.load_texture("eeview-live-frame", image, Default::default());
                        self.texture = Some(texture);
                        self.frames += 1;
                        self.last_frame_at = Some(Instant::now());
                    }
                }
                WorkerEvent::Viewer(ViewerEvent::Stats(stats)) => self.stats = stats,
                WorkerEvent::Viewer(ViewerEvent::State(state)) => self.viewer_state = state,
                WorkerEvent::Viewer(ViewerEvent::Error(err)) | WorkerEvent::Error(err) => {
                    self.last_error = Some(err)
                }
                WorkerEvent::Viewer(ViewerEvent::Eos) => {
                    self.viewer_state = ViewerState::Stopped;
                    self.state.locked = false;
                }
                WorkerEvent::Stopped(stats) => {
                    self.stats = stats;
                    self.viewer_state = ViewerState::Stopped;
                    self.state.locked = false;
                }
            }
        }
    }
}

impl eframe::App for OperatorConsoleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events(ctx);

        egui::SidePanel::right("operator-side-panel")
            .resizable(false)
            .min_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Device");
                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.refresh();
                    }
                    if self.state.locked {
                        ui.label("Running");
                    }
                });
                ui.add_enabled_ui(!self.state.locked, |ui| {
                    egui::ComboBox::from_label("Discovered")
                        .selected_text(
                            self.state
                                .selected_device
                                .and_then(|index| self.state.devices.get(index))
                                .map(|device| device.target.device_uri.as_str())
                                .unwrap_or("None"),
                        )
                        .show_ui(ui, |ui| {
                            for (index, device) in self.state.devices.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.state.selected_device,
                                    Some(index),
                                    &device.target.device_uri,
                                );
                            }
                        });
                    ui.text_edit_singleline(&mut self.state.manual_device_uri);
                    ui.text_edit_singleline(&mut self.state.iface);
                    ui.text_edit_singleline(&mut self.state.bind_address);
                    ui.text_edit_singleline(&mut self.state.stream_name);
                    ui.add(egui::DragValue::new(&mut self.state.port).clamp_range(1..=65535));
                    ui.add(egui::DragValue::new(&mut self.state.timeout_ms).suffix(" ms"));
                    ui.add(egui::DragValue::new(&mut self.state.source_timeout_ms).suffix(" ms"));
                    ui.add(egui::DragValue::new(&mut self.state.latency_ms).suffix(" ms"));
                    ui.add(egui::DragValue::new(&mut self.state.max_packet_size));
                    ui.add(egui::DragValue::new(&mut self.state.packet_delay_ns).suffix(" ns"));
                    ui.checkbox(&mut self.state.recording.enabled, "Record");
                    let mut record_path = self.state.recording.path.to_string_lossy().to_string();
                    if ui.text_edit_singleline(&mut record_path).changed() {
                        self.state.recording.path = PathBuf::from(record_path);
                    }
                });

                ui.separator();
                ui.heading("Diagnostics");
                ui.label(format!("State: {:?}", self.viewer_state));
                ui.label(format!("Frames: {}", self.frames));
                ui.label(format!("Received: {}", self.stats.frames_received));
                ui.label(format!("Dropped: {}", self.stats.frames_dropped));
                ui.label(format!("Anomalies: {}", self.stats.packet_anomalies));
                if let Some(last_frame_at) = self.last_frame_at {
                    ui.label(format!("Last frame: {:.1}s ago", last_frame_at.elapsed().as_secs_f32()));
                }
                if let Some(err) = &self.last_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
            });

        egui::TopBottomPanel::bottom("stream-controls").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if ui
                    .add_enabled(self.state.can_start(), egui::Button::new("Start"))
                    .clicked()
                {
                    self.start();
                }
                if ui
                    .add_enabled(self.state.locked, egui::Button::new("Stop"))
                    .clicked()
                {
                    self.stop();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            if let Some(texture) = &self.texture {
                ui.image((texture.id(), rect.size()));
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("No video");
                });
            }
        });
    }
}

pub fn run() -> eframe::Result<()> {
    gstreamer::init().expect("failed to initialize GStreamer");
    gsteevideo::register_static().expect("failed to register EEVideo plugin");
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "EEVideo Operator Console",
        options,
        Box::new(|_cc| Box::new(OperatorConsoleApp::new())),
    )
}

fn worker_loop(rx: Receiver<WorkerCommand>, tx: Sender<WorkerEvent>) {
    let mut viewer = None::<ViewerPipeline>;
    while let Ok(command) = rx.recv() {
        match command {
            WorkerCommand::Refresh(state) => {
                let controller = DeviceController::new(CoapRegisterBackendConfig {
                    interface_name: empty_to_none(&state.iface),
                    bind_address: Some(state.bind_address),
                    discovery_timeout: Duration::from_millis(state.timeout_ms),
                    request_timeout: Duration::from_millis(state.timeout_ms),
                    yaml_root: None,
                    local_port: 0,
                });
                match controller.discover(None) {
                    Ok(devices) => tx.send(WorkerEvent::Devices(devices)).ok(),
                    Err(err) => tx.send(WorkerEvent::Error(err.to_string())).ok(),
                };
            }
            WorkerCommand::Start(config, backend_config) => {
                let controller = DeviceController::new(backend_config);
                match ViewerPipeline::start(config, controller.shared_backend()) {
                    Ok(pipeline) => {
                        while let Ok(event) = pipeline.events().try_recv() {
                            tx.send(WorkerEvent::Viewer(event)).ok();
                        }
                        viewer = Some(pipeline);
                    }
                    Err(err) => {
                        tx.send(WorkerEvent::Error(format!("{err:#}"))).ok();
                    }
                };
            }
            WorkerCommand::Stop => {
                if let Some(pipeline) = viewer.take() {
                    match pipeline.stop() {
                        Ok(stats) => tx.send(WorkerEvent::Stopped(stats)).ok(),
                        Err(err) => tx.send(WorkerEvent::Error(format!("{err:#}"))).ok(),
                    };
                }
            }
        }

        if let Some(pipeline) = viewer.as_ref() {
            while let Ok(event) = pipeline.events().try_recv() {
                tx.send(WorkerEvent::Viewer(event)).ok();
            }
        }
    }
}

fn empty_to_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
