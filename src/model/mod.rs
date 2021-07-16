pub mod youtubedl;

use std::{
    fs::canonicalize,
    io::Error,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

pub fn upload_file(file: &PathBuf, safe_name: &str) -> Result<String, String> {
    let file = match canonicalize(&file) {
        Ok(path) => path,
        Err(why) => {
            return Err(format!("Couldn't canonicalize path to file, {:?}", why));
        }
    };

    if file.is_dir() {
        return Err("Can't upload a directory".to_string());
    }

    let extension = match file.extension() {
        Some(name) => match name.to_str() {
            Some(name) => name,
            None => {
                return Err(format!("Couldn't get file name of {:?}", file));
            }
        },
        None => {
            return Err(format!("Couldn't get file name of {:?}", file));
        }
    };

    let file = match file.to_str() {
        Some(path) => path,
        None => {
            return Err(format!("Couldn't convert {:?} to a string", file));
        }
    };

    let output = match run_upload(file, safe_name, extension) {
        Err(why) => {
            return Err(format!("Couldn't upload file, {:?}", why));
        }
        Ok(output) => output,
    };

    let output = String::from_utf8(output.stdout).expect("Couldn't convert output of curl");

    Ok(output)
}

fn run_upload(file: &str, file_name: &str, extension: &str) -> Result<Output, Error> {
    let mut cmd = Command::new("curl");

    cmd.env("LC_ALL", "en_US.UTF-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-H")
        .arg("Max-Days: 1")
        .arg("--upload-file")
        .arg(file)
        .arg(format!("http://transfer.sh/{}.{}", file_name, extension));

    match cmd.spawn() {
        Err(why) => {
            return Err(why);
        }
        Ok(process) => process,
    }
    .wait_with_output()
}
