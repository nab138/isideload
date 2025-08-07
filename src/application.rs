// This file was made using https://github.com/Dadoum/Sideloader as a reference.

use crate::bundle::Bundle;
use std::fs::File;
use std::path::PathBuf;
use zip::ZipArchive;

pub struct Application {
    pub bundle: Bundle,
    //pub temp_path: PathBuf,
}

impl Application {
    pub fn new(path: PathBuf) -> Self {
        if !path.exists() {
            panic!("Application path does not exist: {}", path.display());
        }

        let mut bundle_path = path.clone();
        //let mut temp_path = PathBuf::new();

        if path.is_file() {
            let temp_dir = std::env::temp_dir();
            let temp_path = temp_dir.join(path.file_name().unwrap());
            if temp_path.exists() {
                std::fs::remove_dir_all(&temp_path)
                    .expect("Failed to remove existing temporary files");
            }
            std::fs::create_dir_all(&temp_path).expect("Failed to create temporary directory");

            let file = File::open(&path).expect("Failed to open application file");
            let mut archive = ZipArchive::new(file).expect("Failed to read application archive");
            archive
                .extract(&temp_path)
                .expect("Failed to extract application archive");

            let payload_folder = temp_path.join("Payload");
            if payload_folder.exists() && payload_folder.is_dir() {
                let app_dirs: Vec<_> = std::fs::read_dir(&payload_folder)
                    .expect("Failed to read Payload directory")
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "app"))
                    .collect();
                if app_dirs.len() == 1 {
                    bundle_path = app_dirs[0].path();
                } else if app_dirs.is_empty() {
                    panic!("No .app directory found in Payload");
                } else {
                    panic!("Multiple .app directories found in Payload");
                }
            } else {
                panic!("No Payload directory found in the application archive");
            }
        }
        let bundle = Bundle::new(bundle_path).expect("Failed to create application bundle");

        Application {
            bundle, /*temp_path*/
        }
    }
}
