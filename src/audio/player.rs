// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cell::RefCell, rc::Rc};

use glib::{clone, Receiver, Sender};
use gtk::glib;

use crate::{
    audio::{Controller, GstBackend, InhibitController, MprisController, PlayerState, Queue, Song},
    window::WindowAction,
};

#[derive(Clone, Debug)]
pub enum PlaybackAction {
    Play,
    Pause,
    Stop,
    SkipPrevious,
    SkipNext,

    UpdatePosition(u64),
    PlayNext,

    Raise,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

impl Default for PlaybackState {
    fn default() -> Self {
        PlaybackState::Stopped
    }
}

#[derive(Clone, Copy, Debug, glib::Enum, PartialEq)]
#[enum_type(name = "AmberolRepeatMode")]
pub enum RepeatMode {
    Consecutive,
    RepeatAll,
    RepeatOne,
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::Consecutive
    }
}

#[derive(Debug)]
pub enum SeekDirection {
    Forward,
    Backwards,
}

pub struct AudioPlayer {
    window_sender: Sender<WindowAction>,
    receiver: RefCell<Option<Receiver<PlaybackAction>>>,
    backend: GstBackend,
    controllers: Vec<Box<dyn Controller>>,
    queue: Queue,
    state: PlayerState,
}

impl AudioPlayer {
    pub fn new(window_sender: Sender<WindowAction>) -> Rc<Self> {
        let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let receiver = RefCell::new(Some(r));

        let mut controllers: Vec<Box<dyn Controller>> = Vec::new();

        let mpris_controller = MprisController::new(sender.clone());
        controllers.push(Box::new(mpris_controller));

        let inhibit_controller = InhibitController::new();
        controllers.push(Box::new(inhibit_controller));

        let backend = GstBackend::new(sender);

        let queue = Queue::default();
        let state = PlayerState::default();

        let res = Rc::new(Self {
            window_sender,
            receiver,
            backend,
            controllers,
            queue,
            state,
        });

        res.clone().setup_channel();

        res
    }

    fn setup_channel(self: Rc<Self>) {
        let receiver = self.receiver.borrow_mut().take().unwrap();
        receiver.attach(
            None,
            clone!(@strong self as this => move |action| this.clone().process_action(action)),
        );
    }

    fn process_action(&self, action: PlaybackAction) -> glib::Continue {
        match action {
            PlaybackAction::Play => self.set_playback_state(PlaybackState::Playing),
            PlaybackAction::Pause => self.set_playback_state(PlaybackState::Paused),
            PlaybackAction::Stop => self.set_playback_state(PlaybackState::Stopped),
            PlaybackAction::SkipPrevious => self.skip_previous(),
            PlaybackAction::SkipNext => self.skip_next(),
            PlaybackAction::UpdatePosition(pos) => self.update_position(pos),
            PlaybackAction::PlayNext => self.play_next(),
            PlaybackAction::Raise => self.present(),
            // _ => debug!("Received action {:?}", action),
        }

        glib::Continue(true)
    }

    fn set_playback_state(&self, state: PlaybackState) {
        let current_song = self.state.current_song();
        if current_song.is_none() {
            debug!("Getting the first song");
            if let Some(next_song) = self.queue.next_song() {
                debug!("Next song: {}", next_song.uri());

                for c in &self.controllers {
                    c.set_song_artist(&next_song.artist());
                    c.set_song_title(&next_song.title());
                    c.set_song_album(&next_song.album());
                }

                next_song.set_playing(true);

                self.backend.set_song_uri(Some(&next_song.uri()));
                self.state.set_current_song(Some(next_song));
            } else {
                self.backend.set_song_uri(None);
                return;
            }
        } else {
            let current_song = current_song.unwrap();
            debug!("Current song: {}", current_song.uri());
        }

        self.state.set_playback_state(&state);

        for c in &self.controllers {
            c.set_playback_state(&state);
        }

        match state {
            PlaybackState::Playing => self.backend.play(),
            PlaybackState::Paused => self.backend.pause(),
            PlaybackState::Stopped => self.backend.stop(),
        }
    }

    fn play_next(&self) {
        self.skip_next();
    }

    pub fn toggle_play(&self) {
        if self.state.playing() {
            self.set_playback_state(PlaybackState::Paused);
        } else {
            self.set_playback_state(PlaybackState::Playing);
        }
    }

    pub fn stop(&self) {
        self.set_playback_state(PlaybackState::Stopped);
    }

    pub fn skip_previous(&self) {
        if let Some(current_song) = self.state.current_song() {
            // We only skip to the previous song if we are
            // within a seek backward step, otherwise we just
            // restart the song
            if self.state.position() >= 10 {
                self.backend.seek_start();
                return;
            }

            if self.queue.is_first_song() {
                return;
            }

            debug!("Marking '{}' as not playing", current_song.uri());
            current_song.set_playing(false);
        }

        if let Some(prev_song) = self.queue.previous_song() {
            debug!("Playing previous: {}", prev_song.uri());

            let was_playing = self.state.playing();
            if was_playing {
                self.set_playback_state(PlaybackState::Paused);
            }

            for c in &self.controllers {
                c.set_song_artist(&prev_song.artist());
                c.set_song_title(&prev_song.title());
                c.set_song_album(&prev_song.album());
            }

            self.backend.set_song_uri(Some(&prev_song.uri()));
            self.backend.seek_start();

            debug!("Marking '{}' as playing", prev_song.uri());
            prev_song.set_playing(true);

            self.state.set_current_song(Some(prev_song));

            if was_playing {
                self.set_playback_state(PlaybackState::Playing);
            }
        }
    }

    pub fn skip_next(&self) {
        if let Some(current_song) = self.state.current_song() {
            current_song.set_playing(false);
        }

        if let Some(next_song) = self.queue.next_song() {
            debug!("Playing next: {}", next_song.uri());

            let was_playing = self.state.playing();
            if was_playing {
                self.set_playback_state(PlaybackState::Paused);
            }

            for c in &self.controllers {
                c.set_song_artist(&next_song.artist());
                c.set_song_title(&next_song.title());
                c.set_song_album(&next_song.album());
            }

            self.backend.set_song_uri(Some(&next_song.uri()));
            self.backend.seek_start();

            next_song.set_playing(true);

            self.state.set_current_song(Some(next_song));

            if was_playing {
                self.set_playback_state(PlaybackState::Playing);
            }
        } else {
            self.backend.set_song_uri(None);
            self.state.set_current_song(None);
            self.set_playback_state(PlaybackState::Stopped);
        }
    }

    pub fn skip_to(&self, pos: u32) {
        if let Some(current_song) = self.state.current_song() {
            current_song.set_playing(false);
        }

        if let Some(song) = self.queue.skip_song(pos) {
            debug!("Playing next: {}", song.uri());
            let was_playing = self.state.playing();
            if was_playing {
                self.set_playback_state(PlaybackState::Paused);
            }

            for c in &self.controllers {
                c.set_song_artist(&song.artist());
                c.set_song_title(&song.title());
                c.set_song_album(&song.album());
            }

            self.backend.set_song_uri(Some(&song.uri()));
            self.backend.seek_start();

            song.set_playing(true);

            self.state.set_current_song(Some(song));

            if was_playing {
                self.set_playback_state(PlaybackState::Playing);
            }
        } else {
            self.backend.set_song_uri(None);
            self.state.set_current_song(None);
            self.set_playback_state(PlaybackState::Stopped);
        }
    }

    fn seek(&self, offset: u64, direction: SeekDirection) {
        self.backend.seek(
            self.state.position(),
            self.state.duration(),
            offset,
            direction,
        );
    }

    pub fn seek_start(&self) {
        let position = self.state.position() + 1;
        self.backend.seek(
            position,
            self.state.duration(),
            position,
            SeekDirection::Backwards,
        );
    }

    pub fn seek_backwards(&self) {
        self.seek(10, SeekDirection::Backwards);
    }

    pub fn seek_forward(&self) {
        self.seek(10, SeekDirection::Forward);
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn state(&self) -> &PlayerState {
        &self.state
    }

    pub fn set_current_song(&self, song: Option<Song>) {
        self.state.set_current_song(song);
    }

    fn update_position(&self, position: u64) {
        self.state.set_position(position);
    }

    pub fn toggle_repeat_mode(&self) {
        let cur_mode = self.queue.repeat_mode();
        let new_mode = match cur_mode {
            RepeatMode::Consecutive => RepeatMode::RepeatAll,
            RepeatMode::RepeatAll => RepeatMode::RepeatOne,
            RepeatMode::RepeatOne => RepeatMode::Consecutive,
        };
        self.queue.set_repeat_mode(new_mode);
    }

    fn present(&self) {
        send!(self.window_sender, WindowAction::Present);
    }
}