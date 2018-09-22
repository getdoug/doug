use serde_json;
use serde_json::Error;
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Doug settings that are stored on disk
#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    /// Specify default location for data file
    pub data_location: PathBuf,
}

impl Settings {
    /// Load settings.
    /// If the settings file doesn't exist, it will be created.
    pub fn new(folder: &PathBuf) -> Result<Self, String> {
        DirBuilder::new()
            .recursive(true)
            .create(&folder)
            .map_err(|err| format!("Couldn't create data directory: {:?}\n", err))?;

        // create settings file
        let location = folder.as_path().join("settings.json");
        let data_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&location)
            .map_err(|err| format!("Couldn't open settings file: {:?}\n", err))?;

        // serialize settings from data file
        let settings: Result<Settings, Error> = serde_json::from_reader(&data_file);

        match settings {
            Ok(settings) => Ok(settings),
            // No settings exist. Create a new settings instance.
            Err(ref error) if error.is_eof() => {
                let settings = Settings {
                    data_location: folder.to_path_buf(),
                };
                Settings::save(&settings, folder)?;
                Ok(settings)
            }
            Err(err) => Err(format!("There was a serialization issue: {:?}\n", err)),
        }
    }

    pub fn save(&self, folder: &PathBuf) -> Result<(), String> {
        let mut data_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&folder.join("settings.json"))
            .map_err(|err| format!("Couldn't open settings file: {:?}\n", err))?;

        let serialized = serde_json::to_string(&self)
            .map_err(|_| "Couldn't serialize data to string".to_string())?;

        data_file
            .write_all(serialized.as_bytes())
            .map_err(|_| "Couldn't write serialized data to file".to_string())?;
        Ok(())
    }

    pub fn clear(&mut self, folder: &PathBuf) -> Result<(), String> {
        OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&folder.join("settings.json"))
            .map_err(|err| format!("Couldn't clear settings file: {:?}\n", err))?;
        Ok(())
    }
}
