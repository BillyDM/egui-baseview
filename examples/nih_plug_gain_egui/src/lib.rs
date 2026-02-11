use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets, EguiState,
};
use std::sync::Arc;

const MIN_WINDOW_WIDTH: u32 = 300;
const MIN_WINDOW_HEIGHT: u32 = 220;

/// The time it takes for the peak meter to decay by 12 dB after switching to complete silence.
const PEAK_METER_DECAY_MS: f64 = 150.0;

/// If you are using message channels, then allocate enough capacity for the expected worse case
/// scenario for your plugin.
const GUI_TO_AUDIO_MSG_CHANNEL_CAPACITY: usize = 512;
const AUDIO_TO_GUI_MSG_CHANNEL_CAPACITY: usize = 128;

pub struct Gain {
    params: Arc<GainParams>,

    /// Needed to normalize the peak meter's response based on the sample rate.
    peak_meter_decay_weight: f32,
    /// The current data for the peak meter. This is stored as an [`Arc`] so we can share it between
    /// the GUI and the audio processing parts. If you have more state to share, then it's a good
    /// idea to put all of that in a struct behind a single `Arc`.
    ///
    /// This is stored as voltage gain.
    peak_meter: Arc<AtomicF32>,

    /// A message channel to send events between the GUI and the audio thread.
    ///
    /// This is optional. If you don't need to pass events, you can omit this field.
    msg_channel: AudioMsgChannel,
    /// Used to demonstrate how to pass heap-allocated data from the GUI to the audio thread.
    heap_data_example: Vec<f32>,

    /// State that is synced between the GUI and the audio thread using a triple buffer.
    /// This can be used as an alternative to the message channel approach. Note, the roles of which
    /// thread has the input and which has the output can be reversed.
    ///
    /// The downside to this approach is that it takes 3x the memory.
    ///
    /// This is optional. If you don't need this, you can omit it.
    triple_buffer_state: triple_buffer::Output<TripleBufferState>,

    /// Temporarily hold on to the initial GUI state until the editor is first opened.
    initial_gui_state: Option<GuiState>,
}

#[derive(Params)]
pub struct GainParams {
    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "gain"]
    pub gain: FloatParam,

    // TODO: Remove this parameter when we're done implementing the widgets
    #[id = "foobar"]
    pub some_int: IntParam,
}

impl Default for Gain {
    fn default() -> Self {
        let (to_audio_tx, from_gui_rx) = rtrb::RingBuffer::new(GUI_TO_AUDIO_MSG_CHANNEL_CAPACITY);
        let (to_gui_tx, from_audio_rx) = rtrb::RingBuffer::new(AUDIO_TO_GUI_MSG_CHANNEL_CAPACITY);

        let (triple_buffer_input, triple_buffer_output) =
            triple_buffer::triple_buffer(&TripleBufferState::default());

        Self {
            params: Arc::new(GainParams::default()),

            peak_meter_decay_weight: 1.0,
            peak_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),

            msg_channel: AudioMsgChannel {
                to_gui_tx,
                from_gui_rx,
                msg_sent: false,
            },
            heap_data_example: Vec::new(),

            triple_buffer_state: triple_buffer_output,

            initial_gui_state: Some(GuiState {
                msg_channel: GuiMsgChannel {
                    to_audio_tx,
                    from_audio_rx,
                },
                triple_buffer_state: triple_buffer_input,
                next_value: 0,
            }),
        }
    }
}

impl Default for GainParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT),

            // See the main gain example for more details
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            some_int: IntParam::new("Something", 3, IntRange::Linear { min: 0, max: 3 }),
        }
    }
}

/// Here you can store any state you need for your GUI.
///
/// This state persists across editor openings.
pub struct GuiState {
    /// A message channel to send events between the GUI and the audio thread.
    ///
    /// This is optional. If you don't need to pass events, you can omit this field.
    msg_channel: GuiMsgChannel,

    /// State that is synced between the GUI and the audio thread using a triple buffer.
    /// This can be used as an alternative the message channel approach. Note, the roles of which
    /// thread has the input and which has the output can be reversed.
    ///
    /// The downside to this approach is that it takes 3x the memory.
    ///
    /// This is optional. If you don't need this, you can omit it.
    triple_buffer_state: triple_buffer::Input<TripleBufferState>,
    next_value: u64,
}

/// A message channel to send events between the GUI and the audio thread.
///
/// This is optional. If you don't need to pass events, you can omit this.
pub struct GuiMsgChannel {
    /// A message channel to send events from the GUI to the audio thread.
    to_audio_tx: rtrb::Producer<GuiToAudioMsg>,
    /// A message channel to receive events from the audio thread.
    from_audio_rx: rtrb::Consumer<AudioToGuiMsg>,
}
/// A message channel to send events between the GUI and the audio thread.
///
/// This is optional. If you don't need to pass events, you can omit this.
pub struct AudioMsgChannel {
    /// A message channel to send events from the audio thread to the GUI thread.
    to_gui_tx: rtrb::Producer<AudioToGuiMsg>,
    /// A message channel to receive events from the GUI thread.
    from_gui_rx: rtrb::Consumer<GuiToAudioMsg>,
    msg_sent: bool,
}

#[derive(Debug)]
pub enum GuiToAudioMsg {
    MessageA,
    MessageWithHeapData(Vec<f32>),
}
#[derive(Debug)]
pub enum AudioToGuiMsg {
    MessageA,
    DropOldHeapData(Vec<f32>),
}

/// State that is synced between the GUI and the audio thread using a triple buffer.
/// This can be used as an alternative the message channel approach. Note, the roles
/// of which thread has the input and which has the output can be reversed.
///
/// The downside to this approach is that it takes 3x the memory.
///
/// This is optional. If you don't need this, you can omit it.
#[derive(Debug, Default, Clone)]
pub struct TripleBufferState {
    value_a: bool,
    value_b: u64,
    some_data: Vec<u32>,
}

impl Plugin for Gain {
    const NAME: &'static str = "Gain (nih_plug_egui)";
    const VENDOR: &'static str = "Moist Plugins GmbH";
    const URL: &'static str = "https://youtu.be/dQw4w9WgXcQ";
    const EMAIL: &'static str = "info@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let peak_meter = self.peak_meter.clone();
        let egui_state = params.editor_state.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            self.initial_gui_state.take().unwrap(),
            Default::default(),
            |_egui_ctx, _queue, _gui_state| {},
            move |egui_ctx, setter, _queue, gui_state| {
                ResizableWindow::new("res-wind")
                    .min_size(Vec2::new(MIN_WINDOW_WIDTH as f32, MIN_WINDOW_HEIGHT as f32))
                    .show(egui_ctx, egui_state.as_ref(), |ui| {
                        // This is a fancy widget that can get all the information it needs to properly
                        // display and modify the parameter from the parametr itself
                        // It's not yet fully implemented, as the text is missing.
                        ui.label("Some random integer");
                        ui.add(widgets::ParamSlider::for_param(&params.some_int, setter));

                        ui.label("Gain");
                        ui.add(widgets::ParamSlider::for_param(&params.gain, setter));

                        ui.label(
                        "Also gain, but with a standard widget. Note that it doesn't properly take the parameter curve into account!",
                        );

                        // This is a simple naive version of a parameter slider that's not aware of how
                        // the parameters work
                        let prev_value = nih_plug::util::gain_to_db(params.gain.value());
                        let mut new_value = prev_value;
                        ui.add(
                            egui::widgets::Slider::new(&mut new_value, -30.0..=30.0).suffix(" dB"),
                        );
                        if new_value != prev_value {
                            setter.begin_set_parameter(&params.gain);
                            setter
                                .set_parameter(&params.gain, nih_plug::util::db_to_gain(new_value));
                            setter.end_set_parameter(&params.gain);
                        }

                        // TODO: Add a proper custom widget instead of reusing a progress bar
                        let peak_meter =
                            util::gain_to_db(peak_meter.load(std::sync::atomic::Ordering::Relaxed));
                        let peak_meter_text = if peak_meter > util::MINUS_INFINITY_DB {
                            format!("{peak_meter:.1} dBFS")
                        } else {
                            String::from("-inf dBFS")
                        };

                        let peak_meter_normalized = (peak_meter + 60.0) / 60.0;
                        ui.allocate_space(egui::Vec2::splat(2.0));
                        ui.add(
                            egui::widgets::ProgressBar::new(peak_meter_normalized)
                                .text(peak_meter_text),
                        );

                        // Demonstrate sending a message to the audio thread.
                        if ui.button("send message").clicked() {
                            if let Err(e) = gui_state.msg_channel.to_audio_tx.push(GuiToAudioMsg::MessageA) {
                                nih_error!("Failed to send message to audio thread: {}", e);
                            }
                        }
                        // Demonstrate receiving messages from the audio thread.
                        while let Ok(msg) = gui_state.msg_channel.from_audio_rx.pop() {
                            nih_log!("Got message from audio thread: {:?}", &msg);
                        }

                        // Demonstrate mutating synced triple buffer state.
                        if ui.button("mutate synced state").clicked() {
                            gui_state.next_value += 1;
                            // Note, `triple_buffer_state.input_buffer_mut()` will not work for syncing state
                            // this way. You must always completely overwrite the state with new data.
                            gui_state.triple_buffer_state.write(TripleBufferState { value_a: false, value_b: gui_state.next_value, some_data: Vec::new() });
                        }
                    });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // After `PEAK_METER_DECAY_MS` milliseconds of pure silence, the peak meter's value should
        // have dropped by 12 dB
        self.peak_meter_decay_weight = 0.25f64
            .powf((buffer_config.sample_rate as f64 * PEAK_METER_DECAY_MS / 1000.0).recip())
            as f32;

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Demonstrate receiving messages from the GUI thread.
        while let Ok(msg) = self.msg_channel.from_gui_rx.pop() {
            match msg {
                GuiToAudioMsg::MessageA => {
                    nih_dbg!("Got MessageA from GUI");
                }
                GuiToAudioMsg::MessageWithHeapData(mut heap_data) => {
                    nih_dbg!("Got MessageWithHeapData from GUI");

                    // Replace the old heap data with the new data.
                    std::mem::swap(&mut self.heap_data_example, &mut heap_data);

                    // Note, you must be careful not to drop heap-allocated data on the audio
                    // thread. Send the old data back to the GUI thread to be deallocated there.
                    if let Err(e) = self
                        .msg_channel
                        .to_gui_tx
                        .push(AudioToGuiMsg::DropOldHeapData(heap_data))
                    {
                        nih_error!("Failed to send message to GUI thread: {}", e);
                    }
                }
            }
        }

        // Demonstrate sending messages to the GUI thread.
        if self.params.editor_state.is_open() && !self.msg_channel.msg_sent {
            if let Err(e) = self.msg_channel.to_gui_tx.push(AudioToGuiMsg::MessageA) {
                nih_error!("Failed to send message to GUI thread: {}", e);
            }

            // Only send the example message once to avoid spamming the GUI.
            self.msg_channel.msg_sent = true;
        }

        // Demonstrate triple buffer usage.
        let state = self.triple_buffer_state.read();
        // Use the state somehow...
        let _ = &state.value_a;
        let _ = &state.value_b;
        let _ = &state.some_data;

        for channel_samples in buffer.iter_samples() {
            let mut amplitude = 0.0;
            let num_samples = channel_samples.len();

            let gain = self.params.gain.smoothed.next();
            for sample in channel_samples {
                *sample *= gain;
                amplitude += *sample;
            }

            // To save resources, a plugin can (and probably should!) only perform expensive
            // calculations that are only displayed on the GUI while the GUI is open
            if self.params.editor_state.is_open() {
                amplitude = (amplitude / num_samples as f32).abs();
                let current_peak_meter = self.peak_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_peak_meter = if amplitude > current_peak_meter {
                    amplitude
                } else {
                    current_peak_meter * self.peak_meter_decay_weight
                        + amplitude * (1.0 - self.peak_meter_decay_weight)
                };

                self.peak_meter
                    .store(new_peak_meter, std::sync::atomic::Ordering::Relaxed)
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Gain {
    const CLAP_ID: &'static str = "com.moist-plugins-gmbh-egui.nih-plug-gain-egui";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A smoothed gain parameter example plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Gain {
    const VST3_CLASS_ID: [u8; 16] = *b"GainGuiYeahBoyyy";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Gain);
nih_export_vst3!(Gain);
