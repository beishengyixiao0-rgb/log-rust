use flate2::write::GzEncoder;
use flate2::Compression;

use std::fs::File;
use std::io::{copy, BufReader};
use std::thread;

pub fn compress_file_async(path: String) {
    thread::spawn(move || {
        let input = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return,
        };

        let output_path = format!("{}.gz", path);

        let output = match File::create(&output_path) {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut encoder = GzEncoder::new(output, Compression::default());

        let mut reader = BufReader::new(input);

        let _ = copy(&mut reader, &mut encoder);

        let _ = encoder.finish();
        let _ = std::fs::remove_file(&path);
        println!("Compressed: {}", output_path);
    });
}
