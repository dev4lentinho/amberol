// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};

use gtk::{gdk, glib, prelude::*, subclass::prelude::*};

use crate::audio::{PlaybackState, Song};

mod imp {
    use glib::{
        ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecObject, ParamSpecString, ParamSpecUInt64,
    };
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug)]
    pub struct PlayerState {
        pub playback_state: Cell<PlaybackState>,
        pub position: Cell<u64>,
        pub current_song: RefCell<Option<Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayerState {
        const NAME: &'static str = "AmberolPlayerState";
        type Type = super::PlayerState;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self {
                playback_state: Cell::new(PlaybackState::Stopped),
                position: Cell::new(0),
                current_song: RefCell::new(None),
            }
        }
    }

    impl ObjectImpl for PlayerState {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecBoolean::new("playing", "", "", false, ParamFlags::READABLE),
                    ParamSpecUInt64::new("position", "", "", 0, u64::MAX, 0, ParamFlags::READABLE),
                    ParamSpecObject::new("song", "", "", Song::static_type(), ParamFlags::READABLE),
                    ParamSpecString::new("title", "", "", None, ParamFlags::READABLE),
                    ParamSpecString::new("artist", "", "", None, ParamFlags::READABLE),
                    ParamSpecString::new("album", "", "", None, ParamFlags::READABLE),
                    ParamSpecUInt64::new("duration", "", "", 0, u64::MAX, 0, ParamFlags::READABLE),
                    ParamSpecObject::new(
                        "cover",
                        "",
                        "",
                        gdk::Texture::static_type(),
                        ParamFlags::READABLE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> glib::Value {
            match pspec.name() {
                "playing" => obj.playing().to_value(),
                "position" => obj.position().to_value(),
                "song" => self.current_song.borrow().to_value(),

                // These are proxies for Song properties
                "title" => obj.title().to_value(),
                "artist" => obj.artist().to_value(),
                "album" => obj.album().to_value(),
                "duration" => obj.duration().to_value(),
                "cover" => obj.cover().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

// PlayerState is a GObject that we can use to bind to
// widgets and other objects; it contains the current
// state of the audio player: song metadata, playback
// position and duration, etc.
glib::wrapper! {
    pub struct PlayerState(ObjectSubclass<imp::PlayerState>);
}

impl PlayerState {
    fn imp(&self) -> &imp::PlayerState {
        imp::PlayerState::from_instance(self)
    }

    pub fn title(&self) -> Option<String> {
        if let Some(song) = &*self.imp().current_song.borrow() {
            return Some(song.title());
        }

        None
    }

    pub fn artist(&self) -> Option<String> {
        if let Some(song) = &*self.imp().current_song.borrow() {
            return Some(song.artist());
        }

        None
    }

    pub fn album(&self) -> Option<String> {
        if let Some(song) = &*self.imp().current_song.borrow() {
            return Some(song.album());
        }

        None
    }

    pub fn duration(&self) -> u64 {
        if let Some(song) = &*self.imp().current_song.borrow() {
            return song.duration();
        }

        0
    }

    pub fn cover(&self) -> Option<gdk::Texture> {
        if let Some(song) = &*self.imp().current_song.borrow() {
            return song.cover_texture();
        }

        None
    }

    pub fn playing(&self) -> bool {
        let playback_state = self.imp().playback_state.get();
        match playback_state {
            PlaybackState::Playing => true,
            _ => false,
        }
    }

    pub fn set_playback_state(&self, playback_state: &PlaybackState) -> bool {
        let old_state = self.imp().playback_state.replace(*playback_state);
        if old_state != *playback_state {
            self.notify("playing");
            return true;
        }

        false
    }

    pub fn current_song(&self) -> Option<Song> {
        match &*self.imp().current_song.borrow() {
            Some(s) => Some(s.clone()),
            None => None,
        }
    }

    pub fn set_current_song(&self, song: Option<Song>) {
        self.imp().current_song.replace(song);
        self.notify("song");
        self.notify("title");
        self.notify("artist");
        self.notify("album");
        self.notify("duration");
        self.notify("cover");
    }

    pub fn position(&self) -> u64 {
        self.imp().position.get()
    }

    pub fn set_position(&self, position: u64) {
        self.imp().position.replace(position);
        self.notify("position");
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        glib::Object::new::<Self>(&[]).expect("Unable to create PlayerState instance")
    }
}